use crate::{LocalNetwork, OnboardingSnapshot};
#[derive(Clone, Default)]
pub(crate) struct DeviceState {
    pub onboarding: OnboardingSnapshot,
    pub local: LocalNetwork,
    pub fault: Option<String>,
    pub effective_revision: u64,
}
