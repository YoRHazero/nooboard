use super::config::*;
use crate::contracts::terminal::*;
use zeroize::Zeroizing;

/// User-provided authentication material; never serialized or included in snapshots.
pub struct SecretInput(Zeroizing<Vec<u8>>);
impl SecretInput {
    pub fn new(value: Vec<u8>) -> Self {
        Self(Zeroizing::new(value))
    }
    pub fn text(value: String) -> Self {
        Self::new(value.into_bytes())
    }
    pub(crate) fn expose(&self) -> &[u8] {
        &self.0
    }
}
impl std::fmt::Debug for SecretInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretInput([REDACTED])")
    }
}
#[derive(Debug)]
pub enum SshAuthInput {
    Keep,
    /// Ask at authentication time; persist only after successful authentication.
    Password,
    ImportPrivateKey {
        private_key: SecretInput,
        passphrase: Option<SecretInput>,
    },
}
#[derive(Debug)]
pub struct SshTargetInput {
    pub name: String,
    pub address: SshAddressSource,
    pub port: u16,
    pub username: String,
    pub auth: SshAuthInput,
}
#[derive(Debug)]
pub struct CommandInput {
    pub name: String,
    pub target: CommandTarget,
    pub command: String,
    /// Complete desired mapping. Omitted keys are deleted; existing keys retain order.
    pub auto_inputs: Vec<CommandAutoInputEdit>,
}
#[derive(Debug)]
pub struct CommandAutoInputEdit {
    pub prefix: String,
    pub value: AutoInputValue,
}
#[derive(Debug)]
pub enum AutoInputValue {
    Keep,
    Set(SecretInput),
}
#[derive(Clone, Debug)]
pub struct TerminalInput {
    pub name: String,
    pub target: TerminalTarget,
    pub dependencies: Vec<DependencyConfig>,
}
#[derive(Clone, Debug)]
pub struct TunnelInput {
    pub name: String,
    pub target: SshTargetConfigId,
    pub dependencies: Vec<DependencyConfig>,
    pub forward: TunnelForwardConfig,
}
