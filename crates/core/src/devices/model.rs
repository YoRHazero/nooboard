use crate::{LocalAddress, NearbyDevice, PairingStage};
use serde::Serialize;
#[derive(Clone, Debug, Serialize)]
pub enum PairingError {
    Protocol,
    Code,
    Timeout,
    Rejected,
    Cancelled,
    Attempts,
    Disconnected,
    Busy,
    Storage,
    Connect,
}
#[derive(Clone, Debug, Serialize)]
pub enum PairingFailure {
    IdentityChanged,
    IncorrectCode { remaining: u8 },
    Storage,
    Network(PairingError),
}
#[derive(Clone, Serialize)]
pub struct PairingSession {
    pub id: String,
    pub incoming: bool,
    pub device_name: String,
    pub noob_id: Option<String>,
    pub stage: PairingStage,
    pub code: Option<String>,
    pub expires_at_ms: i64,
    pub attempts_left: u8,
    pub error: Option<PairingFailure>,
}
impl PairingSession {
    pub fn active(&self) -> bool {
        !matches!(self.stage, PairingStage::Completed | PairingStage::Failed)
    }
}
#[derive(Clone, Serialize, Default)]
pub struct OnboardingSnapshot {
    pub nearby: Vec<NearbyDevice>,
    pub pairing_address: String,
    pub discovery_error: Option<String>,
    pub session: Option<PairingSession>,
}
#[derive(Clone, Default, PartialEq, Eq, Serialize)]
pub struct LocalNetwork {
    pub sync_port: u16,
    pub pairing_port: u16,
    pub addresses: Vec<LocalAddress>,
    pub error: Option<String>,
}
#[derive(Clone, Default)]
pub(crate) struct DeviceState {
    pub onboarding: OnboardingSnapshot,
    pub local: LocalNetwork,
    pub fault: Option<String>,
    pub effective_revision: u64,
}
#[cfg(any(test, feature = "diagnostics"))]
#[derive(Clone)]
pub struct PeerFixture {
    pub noob_id: String,
    pub certificate: Vec<u8>,
    pub confirmed_fingerprint: String,
    pub device_name: String,
    pub address: Option<String>,
}
