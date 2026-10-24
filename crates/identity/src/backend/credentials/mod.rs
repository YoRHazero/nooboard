use crate::{Error as Failure, IdentityOptions, Result, backend::Material};
#[cfg(feature = "native-credentials")]
mod native;

pub(super) fn load(profile: &str, options: &IdentityOptions) -> Result<Material> {
    if profile.is_empty() || profile.len() > 256 || profile.chars().any(char::is_control) {
        return Err(Failure::InvalidInput);
    }
    #[cfg(feature = "native-credentials")]
    {
        native::load(profile, options)
    }
    #[cfg(not(feature = "native-credentials"))]
    {
        let _ = options;
        Err(Failure::UnsupportedStorage)
    }
}

// Only a genuinely absent credential permits creation. Corruption, denied access and
// failed persistence must not silently change the device's identity.
#[cfg(any(feature = "native-credentials", test))]
fn load_or_create(
    read: impl FnOnce() -> Result<Option<zeroize::Zeroizing<Vec<u8>>>>,
    write: impl FnOnce(&[u8]) -> Result<()>,
) -> Result<Material> {
    if let Some(secret) = read()? {
        return Material::from_secret(&secret);
    }
    let identity = Material::generate()?;
    let secret = identity.export_secret();
    write(&secret)?;
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use zeroize::Zeroizing;

    #[test]
    fn persisted_identity_is_restored_without_replacing_the_credential() {
        let saved = RefCell::new(Vec::new());
        let first = load_or_create(
            || Ok(None),
            |secret| {
                *saved.borrow_mut() = secret.to_vec();
                Ok(())
            },
        )
        .unwrap();
        let second = load_or_create(
            || Ok(Some(Zeroizing::new(saved.into_inner()))),
            |_| panic!("must not rewrite"),
        )
        .unwrap();
        assert_eq!(first.public().unwrap().id, second.public().unwrap().id);
        assert_eq!(
            first.public().unwrap().certificate,
            second.public().unwrap().certificate
        );
    }
    #[test]
    fn denied_corrupt_or_unsaved_credentials_never_fall_back_to_a_new_identity() {
        assert!(matches!(
            load_or_create(|| Err(Failure::Credentials), |_| panic!("must not write")),
            Err(Failure::Credentials)
        ));
        assert!(
            load_or_create(
                || Ok(Some(Zeroizing::new(vec![0; 16]))),
                |_| panic!("must not overwrite")
            )
            .is_err()
        );
        assert!(matches!(
            load_or_create(|| Ok(None), |_| Err(Failure::Credentials)),
            Err(Failure::Credentials)
        ));
    }
}
