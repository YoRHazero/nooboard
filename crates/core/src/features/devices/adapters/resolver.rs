use crate::{Error, Result};
pub(crate) async fn addresses(address: &str) -> Result<Vec<std::net::SocketAddr>> {
    if address.len() > 512 {
        return Err(Error::Configuration);
    }
    let values = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::net::lookup_host(address),
    )
    .await
    .map_err(|_| Error::Offline)?
    .map_err(|_| Error::Configuration)?;
    let mut out = Vec::new();
    for value in values {
        if value.port() != 0
            && !value.ip().is_unspecified()
            && !value.ip().is_multicast()
            && !out.contains(&value)
        {
            out.push(value);
        }
        if out.len() == 16 {
            break;
        }
    }
    if out.is_empty() {
        Err(Error::Configuration)
    } else {
        Ok(out)
    }
}
