use crate::{Error, Result, contracts::terminal::*};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncryptedSecret {
    pub version: u32,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}
impl std::fmt::Debug for EncryptedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EncryptedSecret([REDACTED])")
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SshAuthConfig {
    Password { password: Option<EncryptedSecret> },
    PrivateKey { private_key: EncryptedSecret },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshTargetConfig {
    pub id: SshTargetConfigId,
    pub name: String,
    pub address: SshAddressSource,
    pub port: u16,
    pub username: String,
    pub auth: SshAuthConfig,
    pub(crate) auth_revision: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandTarget {
    Local,
    Ssh(SshTargetConfigId),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalTarget {
    Local,
    Ssh(SshTargetConfigId),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandConfig {
    pub id: CommandConfigId,
    pub name: String,
    pub target: CommandTarget,
    pub command: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyConfig {
    pub command_config_id: CommandConfigId,
    pub reuse_policy: CommandReusePolicy,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalConfig {
    pub id: TerminalConfigId,
    pub name: String,
    pub target: TerminalTarget,
    pub dependencies: Vec<DependencyConfig>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelConfig {
    pub id: TunnelConfigId,
    pub name: String,
    pub target: SshTargetConfigId,
    pub dependencies: Vec<DependencyConfig>,
    pub forward: TunnelForwardConfig,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Catalog {
    pub version: u32,
    pub ssh_targets: BTreeMap<SshTargetConfigId, SshTargetConfig>,
    pub commands: BTreeMap<CommandConfigId, CommandConfig>,
    pub terminals: BTreeMap<TerminalConfigId, TerminalConfig>,
    pub tunnels: BTreeMap<TunnelConfigId, TunnelConfig>,
    pub trusted_hosts: BTreeMap<SshTargetConfigId, SshPublicKey>,
    pub options: TerminalServiceOptions,
}
impl Default for Catalog {
    fn default() -> Self {
        Self {
            version: 1,
            ssh_targets: BTreeMap::new(),
            commands: BTreeMap::new(),
            terminals: BTreeMap::new(),
            tunnels: BTreeMap::new(),
            trusted_hosts: BTreeMap::new(),
            options: TerminalServiceOptions::default(),
        }
    }
}
pub(crate) fn name(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.contains('\0') {
        Err(Error::Configuration)
    } else {
        Ok(())
    }
}
fn endpoint(value: &TunnelEndpoint) -> Result<()> {
    name(&value.host)?;
    if value.port == 0 {
        Err(Error::Configuration)
    } else {
        Ok(())
    }
}
impl Catalog {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(Error::Configuration);
        }
        // The service owns option validation; parsing is pure and preserves the same contract.
        TerminalServiceOptions::from_json(
            &serde_json::to_string(&self.options).map_err(|_| Error::Configuration)?,
        )?;
        for (id, target) in &self.ssh_targets {
            if *id != target.id || target.port == 0 {
                return Err(Error::Configuration);
            }
            name(&target.name)?;
            name(&target.username)?;
            match &target.address {
                SshAddressSource::Host(host) => name(host)?,
                SshAddressSource::Device(id) => name(&id.0)?,
            }
            let secret = match &target.auth {
                SshAuthConfig::Password { password } => password.as_ref(),
                SshAuthConfig::PrivateKey { private_key } => Some(private_key),
            };
            if let Some(secret) = secret
                && (secret.version != 1 || secret.nonce.len() != 24 || secret.ciphertext.len() < 16)
            {
                return Err(Error::Configuration);
            }
        }
        for (id, command) in &self.commands {
            if *id != command.id {
                return Err(Error::Configuration);
            }
            name(&command.name)?;
            name(&command.command)?;
            if let CommandTarget::Ssh(id) = command.target {
                self.ssh(id)?;
            }
        }
        for (id, terminal) in &self.terminals {
            if *id != terminal.id {
                return Err(Error::Configuration);
            }
            name(&terminal.name)?;
            if let TerminalTarget::Ssh(id) = terminal.target {
                self.ssh(id)?;
            }
            self.dependencies(&terminal.dependencies)?;
        }
        for (id, tunnel) in &self.tunnels {
            if *id != tunnel.id {
                return Err(Error::Configuration);
            }
            name(&tunnel.name)?;
            self.ssh(tunnel.target)?;
            self.dependencies(&tunnel.dependencies)?;
            match &tunnel.forward {
                TunnelForwardConfig::Local {
                    listen,
                    destination,
                }
                | TunnelForwardConfig::Remote {
                    listen,
                    destination,
                } => {
                    endpoint(listen)?;
                    endpoint(destination)?;
                }
                TunnelForwardConfig::Dynamic { listen } => endpoint(listen)?,
            }
        }
        for (id, key) in &self.trusted_hosts {
            self.ssh(*id)?;
            if key.algorithm.is_empty() || key.bytes.is_empty() {
                return Err(Error::Configuration);
            }
        }
        Ok(())
    }
    pub fn ssh(&self, id: SshTargetConfigId) -> Result<&SshTargetConfig> {
        self.ssh_targets.get(&id).ok_or(Error::NotFound)
    }
    fn dependencies(&self, dependencies: &[DependencyConfig]) -> Result<()> {
        if dependencies
            .iter()
            .any(|d| !self.commands.contains_key(&d.command_config_id))
        {
            Err(Error::Configuration)
        } else {
            Ok(())
        }
    }
    pub fn referrers(&self, reference: TerminalConfigReference) -> Vec<TerminalConfigReference> {
        let mut references = Vec::new();
        for command in self.commands.values() {
            if matches!((reference, &command.target), (TerminalConfigReference::SshTarget(id), CommandTarget::Ssh(target)) if id == *target)
            {
                references.push(TerminalConfigReference::Command(command.id));
            }
        }
        for terminal in self.terminals.values() {
            let matched = match reference {
                TerminalConfigReference::SshTarget(id) => {
                    terminal.target == TerminalTarget::Ssh(id)
                }
                TerminalConfigReference::Command(id) => terminal
                    .dependencies
                    .iter()
                    .any(|d| d.command_config_id == id),
                _ => false,
            };
            if matched {
                references.push(TerminalConfigReference::Terminal(terminal.id));
            }
        }
        for tunnel in self.tunnels.values() {
            let matched = match reference {
                TerminalConfigReference::SshTarget(id) => tunnel.target == id,
                TerminalConfigReference::Command(id) => tunnel
                    .dependencies
                    .iter()
                    .any(|d| d.command_config_id == id),
                _ => false,
            };
            if matched {
                references.push(TerminalConfigReference::Tunnel(tunnel.id));
            }
        }
        references
    }
}
/// Frozen complete input; address resolution and host trust remain live capabilities.
#[derive(Clone)]
pub(crate) struct StartPlan {
    pub root: StartKind,
    pub catalog: Catalog,
}
#[derive(Clone)]
pub(crate) enum StartKind {
    LocalTerminal(String),
    Terminal(TerminalConfigId),
    Command(CommandConfigId, CommandReusePolicy),
    Tunnel(TunnelConfigId),
}
impl StartPlan {
    pub fn references(&self) -> Result<Vec<TerminalConfigReference>> {
        let mut references = Vec::new();
        let mut commands = Vec::new();
        match self.root {
            StartKind::LocalTerminal(ref display_name) => name(display_name)?,
            StartKind::Terminal(id) => {
                let c = self.catalog.terminals.get(&id).ok_or(Error::NotFound)?;
                references.push(TerminalConfigReference::Terminal(id));
                if let TerminalTarget::Ssh(id) = c.target {
                    references.push(TerminalConfigReference::SshTarget(id));
                }
                commands.extend(c.dependencies.iter().map(|d| d.command_config_id));
            }
            StartKind::Command(id, _) => commands.push(id),
            StartKind::Tunnel(id) => {
                let c = self.catalog.tunnels.get(&id).ok_or(Error::NotFound)?;
                references.extend([
                    TerminalConfigReference::Tunnel(id),
                    TerminalConfigReference::SshTarget(c.target),
                ]);
                commands.extend(c.dependencies.iter().map(|d| d.command_config_id));
            }
        }
        for id in commands {
            let c = self.catalog.commands.get(&id).ok_or(Error::NotFound)?;
            references.push(TerminalConfigReference::Command(id));
            if let CommandTarget::Ssh(id) = c.target {
                references.push(TerminalConfigReference::SshTarget(id));
            }
        }
        references.sort();
        references.dedup();
        Ok(references)
    }
}
