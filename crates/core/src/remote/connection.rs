use super::{
    model::*,
    runtime::{Pending, Remote},
};
use crate::{Error, Result, configuration::runtime::Change};
use nooboard_ssh::{
    Connect, ConnectScope, ConnectionState, PtySize, TerminalChunk, TerminalReplay,
};
use std::net::{IpAddr, SocketAddr};
use tokio::sync::broadcast;

impl Remote {
    pub async fn connect(&self, login: RemoteLogin) -> Result<String> {
        self.check()?;
        if login.port == 0
            || login.username.trim().is_empty()
            || login.username.len() > 256
            || login.username.chars().any(char::is_control)
        {
            return Err(Error::Configuration);
        }
        // Serialize admission and eligibility changes; the SSH initializer returns
        // immediately and never waits for the user's host-key decision here.
        let mut state = self.inner.lock().await;
        let config = self.configuration.current();
        let peer = config.peers.get(&login.device_id).ok_or(Error::NotFound)?;
        let id = profile_id(&login.device_id, &login.username, login.port);
        if state.pending.values().any(|p| p.profile.id == id) {
            return Err(Error::Busy);
        }
        let scope = if let Some(workspace) = &login.workspace {
            let w = state
                .view
                .workspaces
                .iter()
                .find(|w| &w.id == workspace && !w.closed)
                .ok_or(Error::NotFound)?;
            if w.profile_id != id {
                return Err(Error::Configuration);
            }
            let live = |s: &ConnectionState| {
                matches!(s, ConnectionState::Ready | ConnectionState::Connecting)
            };
            match (live(&w.terminal), live(&w.files)) {
                (true, true) => return Err(Error::Busy),
                (true, false) => ConnectScope::Files,
                (false, true) => ConnectScope::Terminal,
                (false, false) => ConnectScope::Both,
            }
        } else {
            if state.view.workspaces.iter().filter(|w| !w.closed).count() >= 8 {
                return Err(Error::Busy);
            }
            ConnectScope::Both
        };
        let old_key = if state.blocked_profiles.contains(&id) {
            None
        } else {
            config
                .remote
                .profiles
                .get(&id)
                .and_then(|p| p.credential.clone())
        };
        let profile = RemoteProfile {
            id: id.clone(),
            device_id: login.device_id.clone(),
            username: login.username.clone(),
            port: login.port,
            credential: None,
        };
        // Revoke durably BEFORE loading/submitting any credential. A crash or a
        // failed connection can never leave this attempt eligible for one click.
        self.configuration
            .change(Change::Remote(ConfigurationChange::Profile(
                profile.clone(),
            )))
            .await?;
        self.publish(&mut state);
        let password = match login.password {
            Some(password) => password,
            None => {
                let key = old_key.clone().ok_or(nooboard_ssh::Error::Credentials)?;
                self.credentials.load(key).await?
            }
        };
        if let Some(key) = old_key {
            let _ = self.credentials.delete(key).await;
        }
        let mut addresses = Vec::new();
        for nearby in self
            .network
            .status()
            .nearby
            .iter()
            .filter(|n| n.noob_id == login.device_id)
        {
            for ip in &nearby.addresses {
                if let Some(address) = discovered_address(ip, login.port) {
                    addresses.push(address);
                }
            }
        }
        if let Some(address) = &peer.settings.address
            && let Ok(resolved) = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                tokio::net::lookup_host(address),
            )
            .await
            && let Ok(resolved) = resolved
        {
            addresses.extend(resolved.map(|mut a| {
                a.set_port(login.port);
                a
            }));
        }
        addresses.extend(
            peer.trusted
                .addresses
                .iter()
                .map(|a| SocketAddr::new(a.ip(), login.port)),
        );
        let mut seen = std::collections::HashSet::new();
        addresses.retain(|a| seen.insert(*a));
        addresses.truncate(64);
        if addresses.is_empty() {
            return Err(Error::Offline);
        }
        let retained = login.remember.then(|| password.duplicate());
        let attempt = self
            .ssh
            .connect(Connect {
                device_id: login.device_id.clone(),
                addresses,
                username: login.username.clone(),
                password,
                trusted_key: config
                    .remote
                    .hosts
                    .get(&host_id(&login.device_id, login.port))
                    .cloned(),
                size: PtySize::default(),
                scope,
            })
            .await?;
        state.attempt_profiles.insert(attempt.clone(), id.clone());
        state.pending.insert(
            attempt.clone(),
            Pending {
                profile,
                password: retained,
            },
        );
        let workspace = if let Some(workspace) = login.workspace {
            let w = state
                .view
                .workspaces
                .iter_mut()
                .find(|w| w.id == workspace)
                .ok_or(Error::NotFound)?;
            if scope != ConnectScope::Files {
                w.terminal_id = attempt.clone();
                w.terminal = ConnectionState::Connecting;
            }
            if scope != ConnectScope::Terminal {
                let queries: Vec<_> = w
                    .directories
                    .iter()
                    .filter_map(|d| d.query.clone())
                    .collect();
                for query in queries {
                    let _ = self.ssh.cancel_directory(query).await;
                }
                w.files_id = attempt;
                w.files = ConnectionState::Connecting;
                w.root = None;
                w.initial_directory = None;
                w.generation += 1;
                w.navigation += 1;
                std::sync::Arc::make_mut(&mut w.directories).clear();
            }
            w.error = None;
            workspace
        } else {
            let workspace = uuid::Uuid::new_v4().to_string();
            state.view.workspaces.push(RemoteWorkspace {
                id: workspace.clone(),
                profile_id: id,
                device_id: login.device_id,
                username: login.username,
                port: login.port,
                address: None,
                terminal_id: attempt.clone(),
                files_id: attempt,
                terminal: ConnectionState::Connecting,
                files: ConnectionState::Connecting,
                initial_directory: None,
                root: None,
                generation: 0,
                navigation: 0,
                directories: Default::default(),
                error: None,
                closed: false,
            });
            workspace
        };
        self.publish(&mut state);
        Ok(workspace)
    }
    pub async fn decide_host_key(&self, request: String, accept: bool) -> Result<()> {
        self.check()?;
        let mut state = self.inner.lock().await;
        let challenge = state
            .view
            .challenges
            .iter()
            .find(|c| c.id == request)
            .cloned()
            .ok_or(Error::NotFound)?;
        if accept {
            let workspace = state
                .view
                .workspaces
                .iter()
                .find(|w| {
                    !w.closed
                        && (w.terminal_id == challenge.session_id
                            || w.files_id == challenge.session_id)
                })
                .ok_or(Error::NotFound)?;
            let id = host_id(&workspace.device_id, workspace.port);
            // A concurrent first connection may have pinned a different key.
            if self
                .configuration
                .current()
                .remote
                .hosts
                .get(&id)
                .is_some_and(|k| k.public_key != challenge.key.public_key)
            {
                return Err(nooboard_ssh::Error::HostKeyChanged.into());
            }
            self.configuration
                .change(Change::Remote(ConfigurationChange::Host(
                    id,
                    Some(challenge.key),
                )))
                .await?;
        }
        self.ssh.decide_host_key(request.clone(), accept).await?;
        state.view.challenges.retain(|c| c.id != request);
        self.publish(&mut state);
        Ok(())
    }
    /// Explicit trust reset. The next connection asks for its new fingerprint.
    pub async fn forget_host_key(&self, device: String, port: u16) -> Result<()> {
        self.check()?;
        if !self.configuration.current().peers.contains_key(&device) {
            return Err(Error::NotFound);
        }
        self.configuration
            .change(Change::Remote(ConfigurationChange::Host(
                host_id(&device, port),
                None,
            )))
            .await
    }
    pub async fn close_workspace(&self, id: String) -> Result<()> {
        self.check()?;
        let mut state = self.inner.lock().await;
        let w = state
            .view
            .workspaces
            .iter_mut()
            .find(|w| w.id == id)
            .ok_or(Error::NotFound)?;
        w.closed = true;
        let queries: Vec<_> = w
            .directories
            .iter()
            .filter_map(|d| d.query.clone())
            .collect();
        for query in queries {
            let _ = self.ssh.cancel_directory(query).await;
        }
        std::sync::Arc::make_mut(&mut w.directories).clear();
        let _ = self.ssh.close_terminal(w.terminal_id.clone()).await;
        // Busy file connections are released by refresh after their tasks finish.
        if !self
            .ssh
            .snapshot()
            .transfers
            .iter()
            .any(|t| t.session_id == w.files_id && !t.state.finished())
        {
            let _ = self.ssh.close_files(w.files_id.clone()).await;
        }
        let attempt_ids = [w.terminal_id.clone(), w.files_id.clone()];
        for id in attempt_ids {
            state.pending.remove(&id);
        }
        for challenge in
            state.view.challenges.iter().filter(|c| {
                state.view.workspaces.iter().any(|w| {
                    w.closed && (w.terminal_id == c.session_id || w.files_id == c.session_id)
                })
            })
        {
            let _ = self.ssh.decide_host_key(challenge.id.clone(), false).await;
        }
        self.publish(&mut state);
        Ok(())
    }
    pub fn subscribe_terminal(&self) -> broadcast::Receiver<TerminalChunk> {
        self.ssh.subscribe_terminal()
    }
    pub async fn terminal_replay(
        &self,
        workspace: String,
        session: String,
        after: u64,
    ) -> Result<TerminalReplay> {
        let id = self.terminal_id(&workspace, &session).await?;
        Ok(self.ssh.terminal_replay(&id, after)?)
    }
    async fn terminal_id(&self, workspace: &str, session: &str) -> Result<String> {
        self.check()?;
        self.inner
            .lock()
            .await
            .view
            .workspaces
            .iter()
            .find(|w| w.id == workspace && !w.closed && w.terminal_id == session)
            .map(|w| w.terminal_id.clone())
            .ok_or(Error::NotFound)
    }
    pub async fn write_terminal(
        &self,
        workspace: String,
        session: String,
        bytes: Vec<u8>,
    ) -> Result<()> {
        if bytes.len() > 64 * 1024 {
            return Err(Error::Busy);
        }
        Ok(self
            .ssh
            .write_terminal(self.terminal_id(&workspace, &session).await?, bytes)
            .await?)
    }
    pub async fn resize_terminal(
        &self,
        workspace: String,
        session: String,
        size: PtySize,
    ) -> Result<()> {
        Ok(self
            .ssh
            .resize_terminal(self.terminal_id(&workspace, &session).await?, size)
            .await?)
    }
}

/// Discovery advertises pairing socket addresses, not bare IP strings. Always
/// replace the advertised port while preserving IPv6 address/scope information.
pub(crate) fn discovered_address(address: &str, ssh_port: u16) -> Option<SocketAddr> {
    if let Ok(mut address) = address.parse::<SocketAddr>() {
        address.set_port(ssh_port);
        Some(address)
    } else {
        address
            .parse::<IpAddr>()
            .ok()
            .map(|ip| SocketAddr::new(ip, ssh_port))
    }
}
