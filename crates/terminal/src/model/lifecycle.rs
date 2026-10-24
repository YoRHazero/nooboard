use super::{CommandConfigId, RunId, SshTargetConfigId, TerminalConfigId, TunnelConfigId};
use serde::{Deserialize, Serialize};

/// Value references retained by a Run's frozen input, independent of any UI subscription.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TerminalConfigReference {
    Terminal(TerminalConfigId),
    Command(CommandConfigId),
    Tunnel(TunnelConfigId),
    SshTarget(SshTargetConfigId),
}
pub trait RunLease: Send + Sync {}
impl<T: Send + Sync> RunLease for T {}

/// Optional composer hook. Called once on registration, before spawning any backend.
/// The returned guard remains owned until the Run and its cleanup have actually ended.
/// Implementations must be short, synchronous, and must not call back into Terminal.
pub trait RunLeaseProvider: Send + Sync {
    fn acquire(&self, run: RunId, references: &[TerminalConfigReference]) -> Box<dyn RunLease>;
}
