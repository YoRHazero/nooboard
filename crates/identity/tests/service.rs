use nooboard_identity::*;

fn context(binding: &str) -> SecretContext {
    SecretContext {
        purpose: "terminal.ssh-auth".into(),
        associated_data: binding.as_bytes().to_vec(),
    }
}
async fn service() -> (IdentityService, Identity) {
    IdentityService::start(IdentityConfig::Ephemeral, IdentityOptions::default())
        .await
        .unwrap()
}

#[tokio::test]
async fn ciphertext_is_randomized_and_bound_to_identity_purpose_and_associated_data() {
    let (mut owner, identity) = service().await;
    let binding = context("config-uuid/password/v1");
    let secret = b"a synthetic test password\0with binary bytes";
    let a = identity
        .seal(binding.clone(), SecretBytes::new(secret.to_vec()))
        .await
        .unwrap();
    let b = identity
        .seal(binding.clone(), SecretBytes::new(secret.to_vec()))
        .await
        .unwrap();
    assert_ne!(a.nonce, b.nonce);
    assert_ne!(a.ciphertext, b.ciphertext);
    assert_eq!(
        identity
            .open(binding.clone(), a.clone())
            .await
            .unwrap()
            .expose_secret(),
        secret
    );
    for other in [
        context("another-config/password/v1"),
        context("config-uuid/private-key/v1"),
        SecretContext {
            purpose: "other-purpose".into(),
            ..binding.clone()
        },
    ] {
        assert_eq!(
            identity.open(other, a.clone()).await.unwrap_err(),
            Error::Authentication
        );
    }
    let (mut other_owner, other) = service().await;
    assert_eq!(
        other.open(binding.clone(), a.clone()).await.unwrap_err(),
        Error::Authentication
    );
    for change_nonce in [false, true] {
        let mut damaged = a.clone();
        if change_nonce {
            damaged.nonce[0] ^= 1;
        } else {
            damaged.ciphertext[0] ^= 1;
        }
        assert_eq!(
            identity.open(binding.clone(), damaged).await.unwrap_err(),
            Error::Authentication
        );
    }
    let mut future = a;
    future.version += 1;
    assert_eq!(
        identity.open(binding, future).await.unwrap_err(),
        Error::UnsupportedVersion
    );
    other_owner.shutdown().await.unwrap();
    owner.shutdown().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn synchronous_tls_signing_does_not_deadlock_the_async_runtime_and_handles_do_not_own_lifetime()
 {
    let (mut owner, identity) = service().await;
    let public = identity.public_identity();
    assert_eq!(public.id.len(), 64);
    let scheme = public.signature_schemes[0];
    assert!(
        !identity
            .sign_blocking(scheme, b"TLS handshake transcript")
            .unwrap()
            .is_empty()
    );
    let clone = identity.clone();
    drop(identity);
    assert!(
        !clone
            .sign(scheme, b"second transcript".to_vec())
            .await
            .unwrap()
            .is_empty()
    );
    owner.shutdown().await.unwrap();
    owner.shutdown().await.unwrap();
    assert_eq!(clone.status().state, ServiceState::Stopped);
    assert_eq!(
        clone.sign_blocking(scheme, b"late transcript").unwrap_err(),
        Error::Stopped
    );
    assert_eq!(
        clone
            .seal(context("late"), SecretBytes::new(vec![1]))
            .await
            .unwrap_err(),
        Error::Stopped
    );
}

#[tokio::test]
async fn dropping_owner_revokes_signing_even_when_a_consumer_retains_the_handle() {
    let (owner, identity) = service().await;
    let scheme = identity.public_identity().signature_schemes[0];
    drop(owner);
    assert_eq!(
        identity.sign_blocking(scheme, b"after drop").unwrap_err(),
        Error::Stopped
    );
    // The worker is independent of the request handle and can exit without dropping it.
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while identity.status().state != ServiceState::Stopped {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn malformed_or_oversized_inputs_are_rejected_without_exposing_secrets() {
    let invalid_options = IdentityOptions {
        queue_capacity: usize::MAX,
        ..Default::default()
    };
    assert_eq!(
        IdentityService::start(IdentityConfig::Ephemeral, invalid_options)
            .await
            .err()
            .unwrap(),
        Error::InvalidInput
    );
    let options = IdentityOptions {
        max_secret_bytes: 8,
        max_signing_bytes: 16,
        ..Default::default()
    };
    let (mut owner, identity) = IdentityService::start(IdentityConfig::Ephemeral, options)
        .await
        .unwrap();
    let secret = SecretBytes::new(b"private input bytes".to_vec());
    assert_eq!(format!("{secret:?}"), "SecretBytes([REDACTED])");
    assert_eq!(
        identity.seal(context("config"), secret).await.unwrap_err(),
        Error::InvalidInput
    );
    assert_eq!(
        identity
            .sign(identity.public_identity().signature_schemes[0], vec![1; 17])
            .await
            .unwrap_err(),
        Error::InvalidInput
    );
    assert_eq!(
        identity
            .seal(context(""), SecretBytes::new(vec![]))
            .await
            .unwrap()
            .ciphertext
            .len(),
        16
    );
    let bad = SecretContext {
        purpose: String::new(),
        associated_data: vec![],
    };
    assert_eq!(
        identity
            .seal(bad, SecretBytes::new(vec![]))
            .await
            .unwrap_err(),
        Error::InvalidInput
    );
    owner.shutdown().await.unwrap();
}

#[cfg(not(feature = "native-credentials"))]
#[tokio::test]
async fn disabling_native_credentials_never_silently_generates_an_ephemeral_identity() {
    let error = IdentityService::start(
        IdentityConfig::System {
            profile: "not-opened-by-this-test".into(),
        },
        Default::default(),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(error, Error::UnsupportedStorage);
}
