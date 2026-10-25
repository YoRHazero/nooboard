use crate::{
    Error, Result,
    contracts::terminal::*,
    features::terminal::{
        application::{
            auth::{AuthSession, Credentials, cancelled},
            requests::{Coordinator, Request},
            state::References,
        },
        domain::config::*,
        ports::{devices::DeviceAddressPort, secrets::SecretsPort},
    },
};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

pub(crate) struct Providers {
    pub coordinator: Coordinator,
    pub devices: Arc<dyn DeviceAddressPort>,
    pub secrets: Arc<dyn SecretsPort>,
}
fn terminal_error(error: Error) -> TerminalError {
    match error {
        Error::Terminal(error) => error,
        Error::Stopped => {
            TerminalError::new(TerminalErrorKind::Cancelled, "application is closing")
        }
        other => TerminalError::new(TerminalErrorKind::Other, other.to_string()),
    }
}
impl Providers {
    fn target(
        self: &Arc<Self>,
        catalog: &Catalog,
        id: SshTargetConfigId,
    ) -> Result<SshStartTarget> {
        let config = catalog.ssh(id)?.clone();
        Ok(SshStartTarget {
            config_id: id,
            config_name: config.name.clone(),
            summary: SshTargetSummary {
                address: config.address.clone(),
                port: config.port,
                username: config.username.clone(),
                auth_kind: match config.auth {
                    SshAuthConfig::Password { .. } => SshAuthKind::Password,
                    SshAuthConfig::PrivateKey { .. } => SshAuthKind::PrivateKey,
                },
            },
            provider: Arc::new(ConnectionProvider {
                config,
                services: self.clone(),
            }),
        })
    }
    pub fn command(
        self: &Arc<Self>,
        catalog: &Catalog,
        id: CommandConfigId,
    ) -> Result<CommandStartSpec> {
        let c = catalog.commands.get(&id).ok_or(Error::NotFound)?;
        Ok(CommandStartSpec {
            config_id: id,
            config_name: c.name.clone(),
            command: c.command.clone(),
            target: match c.target {
                CommandTarget::Local => CommandStartTarget::Local,
                CommandTarget::Ssh(id) => CommandStartTarget::Ssh(self.target(catalog, id)?),
            },
        })
    }
    fn dependencies(
        self: &Arc<Self>,
        catalog: &Catalog,
        dependencies: &[DependencyConfig],
    ) -> Result<Vec<DependencyStartSpec>> {
        dependencies
            .iter()
            .map(|d| {
                Ok(DependencyStartSpec {
                    command: self.command(catalog, d.command_config_id)?,
                    reuse_policy: d.reuse_policy,
                })
            })
            .collect()
    }
    pub fn terminal(
        self: &Arc<Self>,
        catalog: &Catalog,
        id: TerminalConfigId,
    ) -> Result<StartTerminalRequest> {
        let c = catalog.terminals.get(&id).ok_or(Error::NotFound)?;
        Ok(StartTerminalRequest {
            config_id: Some(id),
            name: c.name.clone(),
            target: match c.target {
                TerminalTarget::Local => TerminalStartTarget::Local,
                TerminalTarget::Ssh(id) => TerminalStartTarget::Ssh(self.target(catalog, id)?),
            },
            dependencies: self.dependencies(catalog, &c.dependencies)?,
        })
    }
    pub fn tunnel(
        self: &Arc<Self>,
        catalog: &Catalog,
        id: TunnelConfigId,
    ) -> Result<StartTunnelRequest> {
        let c = catalog.tunnels.get(&id).ok_or(Error::NotFound)?;
        Ok(StartTunnelRequest {
            config_id: id,
            config_name: c.name.clone(),
            target: self.target(catalog, c.target)?,
            forward: c.forward.clone(),
            dependencies: self.dependencies(catalog, &c.dependencies)?,
        })
    }
}
struct ConnectionProvider {
    config: SshTargetConfig,
    services: Arc<Providers>,
}
impl SshConnectionProvider for ConnectionProvider {
    fn prepare(&self) -> BoxFuture<'_, std::result::Result<SshConnectOptions, TerminalError>> {
        Box::pin(async move {
            let address = self
                .services
                .devices
                .resolve(self.config.address.clone(), self.config.port)
                .await
                .map_err(terminal_error)?;
            Ok(SshConnectOptions {
                address,
                username: self.config.username.clone(),
                auth_provider: Arc::new(AuthAdapter(AuthSession {
                    target: self.config.clone(),
                    secrets: self.services.secrets.clone(),
                    coordinator: self.services.coordinator.clone(),
                    password_to_save: Mutex::new(None),
                })),
            })
        })
    }
}
struct AuthAdapter(AuthSession);
impl SshAuthProvider for AuthAdapter {
    fn prepare(
        &self,
        owner: SshConnectionOwnerRunId,
        cancel: CancellationToken,
    ) -> BoxFuture<'_, std::result::Result<SshAuthMaterial, TerminalError>> {
        Box::pin(async move {
            match self
                .0
                .prepare(owner, cancel)
                .await
                .map_err(terminal_error)?
            {
                Credentials::Password(secret) => Ok(SshAuthMaterial::Password(
                    nooboard_terminal::SecretString::new(
                        std::str::from_utf8(secret.expose())
                            .map_err(|_| terminal_error(Error::Configuration))?
                            .to_owned(),
                    ),
                )),
                Credentials::PrivateKey(secret) => Ok(SshAuthMaterial::PrivateKey(
                    nooboard_terminal::SecretBytes::new(secret.expose().to_vec()),
                )),
            }
        })
    }
    fn authentication_succeeded(
        &self,
        _: SshConnectionOwnerRunId,
        cancel: CancellationToken,
    ) -> BoxFuture<'_, ()> {
        Box::pin(self.0.succeeded(cancel))
    }
}
pub(crate) struct HostVerifier(pub Coordinator);
impl SshHostVerifier for HostVerifier {
    fn verify(
        &self,
        request: SshHostVerificationRequest,
        cancel: CancellationToken,
    ) -> BoxFuture<'_, std::result::Result<(), TerminalError>> {
        Box::pin(async move {
            let fingerprint = format!(
                "SHA256:{}",
                base64::engine::general_purpose::STANDARD_NO_PAD
                    .encode(Sha256::digest(&request.server_key.bytes))
            );
            let operation = self
                .0
                .call(|reply| Request::Verify(request, fingerprint, cancel.clone(), reply));
            tokio::select! {biased;_=cancel.cancelled()=>Err(terminal_error(cancelled())),result=operation=>result.map_err(terminal_error)}
        })
    }
}
pub(crate) struct Leases(pub References);
impl RunLeaseProvider for Leases {
    fn acquire(&self, run: RunId, references: &[TerminalConfigReference]) -> Box<dyn RunLease> {
        Box::new(self.0.reserve(Some(run), references))
    }
}
