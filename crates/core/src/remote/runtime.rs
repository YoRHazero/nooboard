use super::model::*;
use crate::{Error, Result, configuration::runtime::Handle as Configuration};
use nooboard_network::Network;
use nooboard_ssh::{ConnectionState, Password, Ssh, SshEvents};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::{Mutex, watch};

pub(crate) struct Pending {
    pub profile: RemoteProfile,
    pub password: Option<Password>,
}
#[derive(Default)]
pub(crate) struct State {
    pub view: RemoteSnapshot,
    pub pending: HashMap<String, Pending>,
    pub failures: HashSet<(String, bool)>,
    pub attempt_profiles: HashMap<String, String>,
    pub blocked_profiles: HashSet<String>,
}
/// Application policy handle. The native AppService owns its lifetime.
#[derive(Clone)]
pub struct Remote {
    pub(crate) ssh: Ssh,
    pub(crate) configuration: Configuration,
    pub(crate) network: Network,
    pub(crate) credentials: Arc<dyn super::credentials::Vault>,
    pub(crate) inner: Arc<Mutex<State>>,
    pub(crate) state: watch::Sender<RemoteSnapshot>,
    pub(crate) stopped: watch::Receiver<bool>,
}
impl Remote {
    pub(crate) fn new(
        ssh: Ssh,
        configuration: Configuration,
        network: Network,
        profile: &str,
        stopped: watch::Receiver<bool>,
    ) -> Self {
        let config = configuration.current().remote;
        let view = RemoteSnapshot {
            profiles: config.profiles.into_values().collect(),
            download_directory: config.download_directory,
            ..Default::default()
        };
        let (state, _) = watch::channel(view.clone());
        Self {
            ssh,
            configuration,
            network,
            credentials: super::credentials::store(profile),
            inner: Arc::new(Mutex::new(State {
                view,
                ..Default::default()
            })),
            state,
            stopped,
        }
    }
    pub fn snapshot(&self) -> RemoteSnapshot {
        self.state.borrow().clone()
    }
    pub fn subscribe(&self) -> watch::Receiver<RemoteSnapshot> {
        self.state.subscribe()
    }
    pub(crate) fn check(&self) -> Result<()> {
        if *self.stopped.borrow() {
            Err(Error::Stopped)
        } else {
            Ok(())
        }
    }
    pub(crate) fn publish(&self, state: &mut State) {
        let config = self.configuration.current().remote;
        let paired = self.configuration.current().peers;
        state.view.profiles = config
            .profiles
            .into_values()
            .filter(|p| paired.contains_key(&p.device_id))
            .map(|mut p| {
                if state.blocked_profiles.contains(&p.id) {
                    p.credential = None;
                }
                p
            })
            .collect();
        state.view.download_directory = config.download_directory;
        state.view.revision += 1;
        let mut view = state.view.clone();
        view.workspaces.retain(|w| !w.closed);
        self.state.send_replace(view);
    }
    pub(crate) async fn run(self, mut events: SshEvents) -> Result<()> {
        let mut ssh = self.ssh.subscribe();
        let mut config = self.configuration.state.clone();
        let mut stop = self.stopped.clone();
        loop {
            tokio::select! { biased;
                _ = stop.changed() => break,
                event = events.next_event() => match event {
                    Some(nooboard_ssh::Event::HostKeyRequired {request_id,session_id,address,key}) => {
                        let mut state = self.inner.lock().await;
                        state.view.challenges.push(RemoteChallenge { id: request_id, session_id, address: address.to_string(), key });
                        self.publish(&mut state);
                    }
                    None => return if *stop.borrow() { Ok(()) } else { Err(Error::Stopped) },
                },
                changed = ssh.changed() => { if changed.is_err() { return Err(Error::Stopped); } self.refresh().await; },
                changed = config.changed() => { if changed.is_err() { break; } self.refresh().await; },
            }
        }
        // Drop any secrets retained until completion. AppService shuts SSH down
        // separately, after business producers finish and before storage closes.
        self.inner.lock().await.pending.clear();
        Ok(())
    }
    async fn refresh(&self) {
        let mut state = self.inner.lock().await;
        let snapshot = self.ssh.snapshot();
        for status in &snapshot.sessions {
            if status.attempt_finished
                && let Some(pending) = state.pending.remove(&status.id)
            {
                let failed = login_failure(&status.terminal) || login_failure(&status.files);
                if status.authenticated
                    && !failed
                    && let Some(password) = pending.password
                {
                    let mut profile = pending.profile;
                    let key = uuid::Uuid::new_v4().to_string();
                    let saved = self.credentials.save(key.clone(), password).await;
                    let saved = match saved {
                        Ok(()) => {
                            state.blocked_profiles.remove(&profile.id);
                            profile.credential = Some(key.clone());
                            self.configuration
                                .change(crate::configuration::runtime::Change::Remote(
                                    ConfigurationChange::Profile(profile),
                                ))
                                .await
                        }
                        Err(e) => Err(e.into()),
                    };
                    if let Err(error) = saved {
                        let _ = self.credentials.delete(key).await;
                        if let Some(w) = state
                            .view
                            .workspaces
                            .iter_mut()
                            .find(|w| w.terminal_id == status.id || w.files_id == status.id)
                        {
                            w.error = Some(error.to_string());
                        }
                    }
                }
            }
            let terminal_failed = login_failure(&status.terminal)
                && state.failures.insert((status.id.clone(), false));
            let files_failed =
                login_failure(&status.files) && state.failures.insert((status.id.clone(), true));
            if (terminal_failed || files_failed)
                && let Some(id) = state
                    .view
                    .workspaces
                    .iter()
                    .find(|w| w.terminal_id == status.id || w.files_id == status.id)
                    .map(|w| w.profile_id.clone())
            {
                // Eligibility was cleared durably before starting this attempt.
                // Later transport failures must also revoke a successfully saved login.
                state.blocked_profiles.insert(id.clone());
                if let Some(mut profile) = self
                    .configuration
                    .current()
                    .remote
                    .profiles
                    .get(&id)
                    .cloned()
                    && let Some(key) = profile.credential.take()
                {
                    let persisted = self
                        .configuration
                        .change(crate::configuration::runtime::Change::Remote(
                            ConfigurationChange::Profile(profile),
                        ))
                        .await;
                    let deleted = self.credentials.delete(key).await;
                    if (persisted.is_err() || deleted.is_err())
                        && let Some(w) = state
                            .view
                            .workspaces
                            .iter_mut()
                            .find(|w| w.profile_id == id)
                    {
                        w.error = Some("Saved login could not be removed completely".into());
                    }
                }
            }
        }
        let paired = self.configuration.current().peers;
        for workspace in &mut state.view.workspaces {
            if !paired.contains_key(&workspace.device_id) {
                workspace.closed = true;
            }
            if let Some(s) = snapshot
                .sessions
                .iter()
                .find(|s| s.id == workspace.terminal_id)
            {
                workspace.terminal = s.terminal.clone();
                workspace.address = s.address.map(|a| a.to_string());
            }
            if let Some(s) = snapshot
                .sessions
                .iter()
                .find(|s| s.id == workspace.files_id)
            {
                workspace.files = s.files.clone();
                if workspace.address.is_none() {
                    workspace.address = s.address.map(|a| a.to_string());
                }
                workspace.initial_directory = s.initial_directory.clone();
                if workspace.root.is_none() {
                    workspace.root = s.initial_directory.clone();
                }
            }
        }
        // Release only orphaned halves. A closed tab keeps its file transport
        // until every submitted task finishes; old reconnect attempts are freed.
        for session in &snapshot.sessions {
            let terminal_used = state
                .view
                .workspaces
                .iter()
                .any(|w| !w.closed && w.terminal_id == session.id);
            let files_used = state
                .view
                .workspaces
                .iter()
                .any(|w| !w.closed && w.files_id == session.id)
                || snapshot
                    .transfers
                    .iter()
                    .any(|t| t.session_id == session.id && !t.state.finished());
            if !terminal_used && matches!(session.terminal, ConnectionState::Ready) {
                let _ = self.ssh.close_terminal(session.id.clone()).await;
            }
            if !files_used && session.attempt_finished && session.files != ConnectionState::Closed {
                let _ = self.ssh.close_files(session.id.clone()).await;
            }
            if !terminal_used && !files_used && session.attempt_finished {
                let _ = self.ssh.forget_session(session.id.clone()).await;
            }
        }
        state.view.workspaces.retain(|w| !w.closed);
        let retained: HashSet<_> = snapshot
            .sessions
            .iter()
            .map(|s| s.id.as_str())
            .chain(snapshot.transfers.iter().map(|t| t.session_id.as_str()))
            .collect();
        state
            .attempt_profiles
            .retain(|id, _| retained.contains(id.as_str()));
        state
            .failures
            .retain(|(id, _)| retained.contains(id.as_str()));
        // Successful uploads invalidate only the affected listing. The UI decides
        // whether an expanded stale directory should be fetched again.
        for transfer in &snapshot.transfers {
            if transfer.state == nooboard_ssh::TransferState::Succeeded
                && !state
                    .view
                    .transfers
                    .iter()
                    .any(|t| t.id == transfer.id && t.state == transfer.state)
                && let nooboard_ssh::TransferSpec::Upload {
                    remote_directory, ..
                } = &transfer.spec
            {
                for w in &mut state.view.workspaces {
                    if w.files_id == transfer.session_id {
                        for d in std::sync::Arc::make_mut(&mut w.directories) {
                            if d.path == *remote_directory {
                                d.generation += 1;
                                d.state = DirectoryState::Stale;
                                d.cursor = None;
                                if let Some(query) = d.query.take() {
                                    let _ = self.ssh.cancel_directory(query).await;
                                }
                            }
                        }
                    }
                }
            }
        }
        state.view.transfers = snapshot.transfers;
        state.view.challenges.retain(|c| {
            snapshot
                .sessions
                .iter()
                .any(|s| s.id == c.session_id && !s.attempt_finished)
        });
        self.publish(&mut state);
    }
}
pub(crate) fn login_failure(state: &ConnectionState) -> bool {
    matches!(state, ConnectionState::Failed(error) if !matches!(error, nooboard_ssh::Error::SftpUnavailable))
}
