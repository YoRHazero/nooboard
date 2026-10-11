//! Stable IPC error codes. Diagnostic details stay out of translated UI messages.
use nooboard_core::{PairingError, PairingFailure};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct UiError {
    code: &'static str,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[schemars(default)]
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
        Error::Ssh(error) => return ssh(error),
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

pub fn ssh(error: nooboard_core::SshError) -> UiError {
    use nooboard_core::SshError as E;
    ui(match error {
        E::Authentication => "sshAuthentication",
        E::HostKeyChanged => "sshHostKeyChanged",
        E::HostKeyRejected => "sshHostKeyRejected",
        E::Timeout => "sshTimeout",
        E::Connection(_) => "sshConnection",
        E::SftpUnavailable => "sshSftpUnavailable",
        E::InvalidPath => "sshInvalidPath",
        E::NotFound => "sshNotFound",
        E::NotDirectory => "sshNotDirectory",
        E::NotFile => "sshNotFile",
        E::PermissionDenied => "sshPermission",
        E::StaleCursor => "sshStaleCursor",
        E::Credentials => "sshCredentials",
        E::Unconfirmed => "sshUnconfirmed",
        E::SourceChanged => "sshSourceChanged",
        E::Io(_) | E::Protocol(_) => "sshIo",
        E::Busy | E::Limit => "busy",
        E::Cancelled => "sshCancelled",
        E::Stopped => "stopped",
        E::Internal => "serviceFault",
    })
}

pub fn remote(error: nooboard_core::Error) -> UiError {
    match error {
        nooboard_core::Error::NotFound => ui("sshCancelled"),
        nooboard_core::Error::Offline => ui("sshConnection"),
        other => core(other),
    }
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
