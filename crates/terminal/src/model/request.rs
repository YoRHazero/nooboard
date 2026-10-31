use super::*;
use crate::{SecretBytes, SecretString};
use std::{future::Future, net::SocketAddr, pin::Pin, sync::Arc};
use tokio_util::sync::CancellationToken;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone, Debug)]
pub struct StartTerminalRequest {
    /// None starts a session without a saved terminal configuration.
    pub config_id: Option<TerminalConfigId>,
    pub name: String,
    pub target: TerminalStartTarget,
    pub dependencies: Vec<DependencyStartSpec>,
}

#[derive(Clone, Debug)]
pub enum TerminalStartTarget {
    Local,
    Ssh(SshStartTarget),
}

#[derive(Clone, Debug)]
pub struct StartTunnelRequest {
    pub config_id: TunnelConfigId,
    pub config_name: String,
    pub target: SshStartTarget,
    pub forward: TunnelForwardConfig,
    pub dependencies: Vec<DependencyStartSpec>,
}

#[derive(Clone, Debug)]
pub struct StartCommandRequest {
    pub command: CommandStartSpec,
    pub reuse_policy: CommandReusePolicy,
}

#[derive(Clone, Debug)]
pub struct DependencyStartSpec {
    pub command: CommandStartSpec,
    pub reuse_policy: CommandReusePolicy,
}

#[derive(Clone, Debug)]
pub struct CommandStartSpec {
    pub config_id: CommandConfigId,
    pub config_name: String,
    pub target: CommandStartTarget,
    pub command: String,
    /// Ordered, frozen rules. Each value is sent at most once per new run.
    pub auto_inputs: Vec<CommandAutoInput>,
}

#[derive(Clone, Debug)]
pub struct CommandAutoInput {
    pub prefix: String,
    pub value: SecretString,
}

#[derive(Clone, Debug)]
pub enum CommandStartTarget {
    Local,
    Ssh(SshStartTarget),
}

#[derive(Clone)]
pub struct SshStartTarget {
    pub config_id: SshTargetConfigId,
    pub config_name: String,
    pub summary: SshTargetSummary,
    pub provider: Arc<dyn SshConnectionProvider>,
}

impl std::fmt::Debug for SshStartTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshStartTarget")
            .field("config_id", &self.config_id)
            .field("config_name", &self.config_name)
            .field("summary", &self.summary)
            .finish_non_exhaustive()
    }
}
pub trait SshConnectionProvider: Send + Sync {
    fn prepare(&self) -> BoxFuture<'_, Result<SshConnectOptions>>;
}

pub struct SshConnectOptions {
    pub address: SocketAddr,
    pub username: String,
    pub auth_provider: Arc<dyn SshAuthProvider>,
}

pub trait SshAuthProvider: Send + Sync {
    fn prepare(
        &self,
        owner_run_id: SshConnectionOwnerRunId,
        cancellation: CancellationToken,
    ) -> BoxFuture<'_, Result<SshAuthMaterial>>;

    fn authentication_succeeded(
        &self,
        owner_run_id: SshConnectionOwnerRunId,
        cancellation: CancellationToken,
    ) -> BoxFuture<'_, ()>;
}

#[derive(Debug)]
pub enum SshAuthMaterial {
    Password(SecretString),
    PrivateKey(SecretBytes),
}

#[derive(Clone, Debug)]
pub struct SshHostVerificationRequest {
    pub owner_run_id: SshConnectionOwnerRunId,
    pub target_config_id: SshTargetConfigId,
    pub address: SocketAddr,
    pub server_key: SshPublicKey,
}

pub trait SshHostVerifier: Send + Sync {
    fn verify(
        &self,
        request: SshHostVerificationRequest,
        cancellation: CancellationToken,
    ) -> BoxFuture<'_, Result<()>>;
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SshPublicKey {
    pub algorithm: String,
    /// SSH wire-format public key bytes, not a fingerprint or a private key.
    pub bytes: Vec<u8>,
}
