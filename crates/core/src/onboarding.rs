//! Recoverable discovery and pairing UI state. Codes are ephemeral and omitted from Debug.
use nooboard_network::pairing::{Control, Endpoint, Stage};
pub use nooboard_network::{discovery::Device as NearbyDevice, pairing::Stage as PairingStage};
use serde::Serialize;
use tokio::sync::{mpsc, watch};
/// Semantic failures remain independent of any desktop language.
#[derive(Clone, Debug, Serialize)]
pub enum PairingFailure {
    IdentityChanged,
    IncorrectCode { remaining: u8 },
    Storage,
    Network(nooboard_network::pairing::Error),
}
pub use nooboard_network::pairing::Error as PairingError;
#[derive(Clone, Serialize)]
pub struct PairingSession {
    pub id: String,
    pub incoming: bool,
    pub device_name: String,
    pub noob_id: Option<String>,
    pub stage: Stage,
    pub code: Option<String>,
    pub expires_at_ms: i64,
    pub attempts_left: u8,
    pub error: Option<PairingFailure>,
}
impl PairingSession {
    pub fn active(&self) -> bool {
        !matches!(self.stage, Stage::Completed | Stage::Failed)
    }
}
#[derive(Clone, Serialize, Default)]
pub struct OnboardingSnapshot {
    pub nearby: Vec<NearbyDevice>,
    pub pairing_address: String,
    pub discovery_error: Option<String>,
    pub session: Option<PairingSession>,
}
pub(crate) struct Onboarding {
    pub endpoint: Endpoint,
    pub discovery: Option<nooboard_network::discovery::Discovery>,
    pub nearby: watch::Receiver<Vec<NearbyDevice>>,
    pub events: mpsc::Receiver<nooboard_network::pairing::Event>,
    pub sender: mpsc::Sender<nooboard_network::pairing::Event>,
    pub control: Option<Control>,
    pub expected: Option<String>,
    pub snapshot: OnboardingSnapshot,
}
