//! Fair address selection across bounded connection rounds.
use crate::error::{Failure, InternalResult as Result};
use std::{future::Future, net::SocketAddr, time::Duration};

const ROUND_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn order(next: &mut usize, mut addresses: Vec<SocketAddr>) -> Vec<SocketAddr> {
    if !addresses.is_empty() {
        let start = *next % addresses.len();
        addresses.rotate_left(start);
        *next = (start + 1) % addresses.len();
    }
    addresses
}

pub(super) async fn connect<T, F: Future<Output = Result<T>>>(
    addresses: Vec<SocketAddr>,
    mut attempt: impl FnMut(SocketAddr) -> F,
) -> Result<T> {
    tokio::time::timeout(ROUND_TIMEOUT, async {
        for address in addresses {
            if let Ok(connection) = attempt(address).await {
                return Ok(connection);
            }
        }
        Err(Failure::Disconnected)
    })
    .await
    .unwrap_or(Err(Failure::Timeout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn repeated_timeouts_do_not_starve_a_working_address_at_the_end() {
        let addresses = (1..=16)
            .map(|port| SocketAddr::from(([127, 0, 0, 1], port)))
            .collect::<Vec<_>>();
        let working = *addresses.last().unwrap();
        let mut next = 0;
        for _ in 0..addresses.len() {
            let result = connect(order(&mut next, addresses.clone()), |address| async move {
                if address == working {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                    Ok(address)
                } else {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    Err(Failure::Timeout)
                }
            })
            .await;
            if let Ok(address) = result {
                assert_eq!(address, working);
                return;
            }
        }
        panic!("the reachable address never received a usable connection window");
    }

    #[test]
    fn cursor_wraps_safely_when_discovery_changes_the_address_list() {
        let a = SocketAddr::from(([127, 0, 0, 1], 1));
        let b = SocketAddr::from(([127, 0, 0, 1], 2));
        let mut next = 15;
        assert!(order(&mut next, vec![]).is_empty());
        assert_eq!(order(&mut next, vec![a, b]), vec![b, a]);
        assert_eq!(order(&mut next, vec![a]), vec![a]);
        assert_eq!(order(&mut next, vec![a, b]), vec![a, b]);
    }
}
