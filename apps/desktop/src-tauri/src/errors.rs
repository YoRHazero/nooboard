//! Stable IPC error codes. Diagnostic details stay out of translated UI messages.
use nooboard_core::{PairingError, PairingFailure};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
pub struct UiError {
    code: &'static str,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    params: BTreeMap<&'static str, u8>,
}
pub fn ui(code: &'static str) -> UiError {
    UiError {
        code,
        params: BTreeMap::new(),
    }
}
pub fn core(error: nooboard_core::Error) -> UiError {
    use nooboard_core::Error;
    ui(match error {
        Error::Network(_) => "network",
        Error::Storage(_) => "storage",
        Error::Clipboard(_) => "clipboard",
        Error::Stopped => "stopped",
        Error::Offline => "offline",
        Error::NoTargets => "noTargets",
        Error::Busy => "busy",
        Error::Paused => "paused",
        Error::Ineligible => "ineligible",
        Error::Configuration => "configuration",
        Error::AlreadyPaired => "identityChanged",
        Error::Fingerprint => "fingerprint",
        Error::NotFound => "historyMissing",
        Error::Internal => "serviceFault",
    })
}
pub fn pairing(error: &PairingFailure) -> UiError {
    let code = match error {
        PairingFailure::IdentityChanged => "pairingIdentityChanged",
        PairingFailure::IncorrectCode { remaining } => {
            return UiError {
                code: "pairingCodeRemaining",
                params: BTreeMap::from([("count", *remaining)]),
            };
        }
        PairingFailure::Storage => "pairingStorage",
        PairingFailure::Network(error) => match error {
            PairingError::Protocol => "pairingProtocol",
            PairingError::Code => "pairingCode",
            PairingError::Timeout => "pairingTimeout",
            PairingError::Rejected => "pairingRejected",
            PairingError::Cancelled => "pairingCancelled",
            PairingError::Attempts => "pairingAttempts",
            PairingError::Disconnected => "pairingDisconnected",
            PairingError::Busy => "pairingUnavailable",
            PairingError::Storage => "pairingStorage",
            PairingError::Connect => "pairingConnect",
        },
    };
    ui(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pairing_failure_carries_a_code_and_count_without_translated_text() {
        assert_eq!(
            serde_json::to_value(pairing(&PairingFailure::IncorrectCode { remaining: 2 })).unwrap(),
            serde_json::json!({"code": "pairingCodeRemaining", "params": {"count": 2}})
        );
        assert_eq!(
            serde_json::to_value(pairing(&PairingFailure::Network(PairingError::Timeout))).unwrap(),
            serde_json::json!({"code": "pairingTimeout"})
        );
    }
}
