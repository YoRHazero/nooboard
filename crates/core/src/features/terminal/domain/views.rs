use super::config::*;
use crate::contracts::terminal::*;
use serde::Serialize;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct TerminalPromptId(pub(crate) uuid::Uuid);
impl std::fmt::Display for TerminalPromptId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::str::FromStr for TerminalPromptId {
    type Err = uuid::Error;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        uuid::Uuid::parse_str(value).map(Self)
    }
}

#[derive(Clone, Debug, Serialize)]
pub enum SshAuthSummary {
    Password { saved: bool },
    PrivateKey,
}
#[derive(Clone, Debug, Serialize)]
pub struct SshTargetView {
    pub id: SshTargetConfigId,
    pub name: String,
    pub address: SshAddressSource,
    pub port: u16,
    pub username: String,
    pub auth: SshAuthSummary,
    pub trusted_key: Option<SshPublicKey>,
}
#[derive(Clone, Debug, Serialize)]
pub enum TerminalPromptKind {
    HostTrust {
        address: std::net::SocketAddr,
        server_key: SshPublicKey,
        fingerprint: String,
    },
    Password {
        username: String,
    },
}
#[derive(Clone, Debug, Serialize)]
pub struct TerminalPrompt {
    pub id: TerminalPromptId,
    pub target_id: SshTargetConfigId,
    pub owner: SshConnectionOwnerRunId,
    pub kind: TerminalPromptKind,
}
#[derive(Clone, Debug, Serialize)]
pub enum TerminalIssue {
    CredentialNotSaved {
        target_id: SshTargetConfigId,
        message: String,
    },
    HostKeyChanged {
        target_id: SshTargetConfigId,
        previous: SshPublicKey,
        received: SshPublicKey,
    },
    Service {
        message: String,
    },
}
#[derive(Clone, Debug, Serialize)]
pub struct TerminalWorkspaceSnapshot {
    pub revision: u64,
    pub closing: bool,
    pub ssh_targets: Vec<SshTargetView>,
    pub commands: Vec<CommandConfigView>,
    pub terminals: Vec<TerminalConfig>,
    pub tunnels: Vec<TunnelConfig>,
    pub runs: TerminalSnapshot,
    pub prompts: Vec<TerminalPrompt>,
    pub issue: Option<TerminalIssue>,
    pub options: TerminalServiceOptions,
}
#[derive(Clone, Debug, Serialize)]
pub struct CommandConfigView {
    pub id: CommandConfigId,
    pub name: String,
    pub target: CommandTarget,
    pub command: String,
    /// Only ordered keys are exposed. Values and ciphertext stay in Core.
    pub auto_inputs: Vec<String>,
}
impl Catalog {
    pub(crate) fn view(&self) -> TerminalWorkspaceSnapshot {
        TerminalWorkspaceSnapshot {
            revision: 0,
            closing: false,
            ssh_targets: self
                .ssh_targets
                .values()
                .map(|c| SshTargetView {
                    id: c.id,
                    name: c.name.clone(),
                    address: c.address.clone(),
                    port: c.port,
                    username: c.username.clone(),
                    auth: match &c.auth {
                        SshAuthConfig::Password { password } => SshAuthSummary::Password {
                            saved: password.is_some(),
                        },
                        SshAuthConfig::PrivateKey { .. } => SshAuthSummary::PrivateKey,
                    },
                    trusted_key: self.trusted_hosts.get(&c.id).cloned(),
                })
                .collect(),
            commands: self
                .commands
                .values()
                .map(|c| CommandConfigView {
                    id: c.id,
                    name: c.name.clone(),
                    target: c.target.clone(),
                    command: c.command.clone(),
                    auto_inputs: c.auto_inputs.keys().cloned().collect(),
                })
                .collect(),
            terminals: self.terminals.values().cloned().collect(),
            tunnels: self.tunnels.values().cloned().collect(),
            runs: TerminalSnapshot::default(),
            prompts: vec![],
            issue: None,
            options: self.options.clone(),
        }
    }
}
