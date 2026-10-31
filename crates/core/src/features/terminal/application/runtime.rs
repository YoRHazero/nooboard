#[path = "config.rs"]
mod config;
#[path = "prompts.rs"]
mod prompts;
#[path = "start.rs"]
mod start;
use super::{
    auth::{PasswordAnswer, cancelled},
    requests::*,
    state::References,
};
use crate::{
    Error, Result,
    contracts::terminal::*,
    features::terminal::{
        domain::{config::*, inputs::*, views::*},
        ports::{
            engine::TerminalEngine,
            keys::PrivateKeyPort,
            repository::TerminalRepository,
            secrets::{SecretField, SecretScope, SecretsPort},
        },
    },
};
use std::{collections::BTreeMap, sync::Arc};
use tokio::{
    sync::{mpsc, watch},
    task::JoinSet,
};

enum PromptReply {
    Host(Reply<()>),
    Password(Reply<PasswordAnswer>),
}
struct Pending {
    view: TerminalPrompt,
    reply: Option<PromptReply>,
    cancellation: CancellationToken,
    done: CancellationToken,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.done.cancel();
    }
}
pub(crate) struct Runtime {
    catalog: Catalog,
    inbox: mpsc::Receiver<Request>,
    state: watch::Sender<TerminalWorkspaceSnapshot>,
    engine: Arc<dyn TerminalEngine>,
    repository: Arc<dyn TerminalRepository>,
    secrets: Arc<dyn SecretsPort>,
    keys: Arc<dyn PrivateKeyPort>,
    references: References,
    pending: BTreeMap<TerminalPromptId, Pending>,
    tasks: JoinSet<()>,
    prompts: JoinSet<Option<TerminalPromptId>>,
    closing: bool,
    issue: Option<TerminalIssue>,
    revision: u64,
}
pub(crate) struct Dependencies {
    pub engine: Arc<dyn TerminalEngine>,
    pub repository: Arc<dyn TerminalRepository>,
    pub secrets: Arc<dyn SecretsPort>,
    pub keys: Arc<dyn PrivateKeyPort>,
    pub references: References,
}
impl Runtime {
    pub fn new(
        catalog: Catalog,
        inbox: mpsc::Receiver<Request>,
        state: watch::Sender<TerminalWorkspaceSnapshot>,
        dependencies: Dependencies,
    ) -> Self {
        Self {
            catalog,
            inbox,
            state,
            engine: dependencies.engine,
            repository: dependencies.repository,
            secrets: dependencies.secrets,
            keys: dependencies.keys,
            references: dependencies.references,
            pending: BTreeMap::new(),
            tasks: JoinSet::new(),
            prompts: JoinSet::new(),
            closing: false,
            issue: None,
            revision: 0,
        }
    }
    pub async fn run(mut self, stop: CancellationToken) {
        let mut runs = self.engine.subscribe().snapshots;
        let mut watching = true;
        loop {
            tokio::select! {
                _ = stop.cancelled() => break,
                request = self.inbox.recv() => match request {
                    Some(Request::Finish) | None => break,
                    Some(request) => self.request(request).await,
                },
                state = runs.next(), if watching => {
                    if state.is_none() {
                        watching = false;
                        if !self.closing {
                            self.closing = true;
                            self.issue = Some(TerminalIssue::Service {
                                message: "terminal service stopped unexpectedly".into(),
                            });
                            self.cancel_all();
                        }
                    }
                }
                done = self.tasks.join_next(), if !self.tasks.is_empty() => {
                    if let Some(Err(error)) = done {
                        self.issue = Some(TerminalIssue::Service { message: error.to_string() });
                    }
                }
                done = self.prompts.join_next(), if !self.prompts.is_empty() => {
                    if let Some(Ok(Some(id))) = done { self.cancel(id); }
                }
            }
            self.publish();
        }
        self.closing = true;
        self.cancel_all();
        self.inbox.close();
        self.prompts.abort_all();
        while self.tasks.join_next().await.is_some() {}
        self.publish();
    }
    fn admit(&self) -> Result<()> {
        if self.closing || !self.engine.is_available() {
            Err(Error::Stopped)
        } else {
            Ok(())
        }
    }
    fn publish(&mut self) {
        self.revision += 1;
        let mut view = self.catalog.view();
        view.revision = self.revision;
        view.closing = self.closing;
        view.runs = self.engine.snapshot();
        view.prompts = self.pending.values().map(|p| p.view.clone()).collect();
        view.issue = self.issue.clone();
        self.state.send_replace(view);
    }
    async fn commit(&mut self, next: Catalog) -> Result<()> {
        next.validate()?;
        self.repository.save(next.clone()).await?;
        self.catalog = next;
        Ok(())
    }
    fn reply<T>(&mut self, reply: Reply<T>, result: Result<T>) {
        self.publish();
        let _ = reply.send(result);
    }
    async fn request(&mut self, request: Request) {
        match request {
            Request::Ssh(id, input, reply) => {
                let result = self.save_ssh(id, input).await;
                self.reply(reply, result);
            }
            Request::Command(id, input, reply) => {
                let result = self.save_command(id, input).await;
                self.reply(reply, result);
            }
            Request::Terminal(id, input, reply) => {
                let result = self.save_terminal(id, input).await;
                self.reply(reply, result);
            }
            Request::Tunnel(id, input, reply) => {
                let result = self.save_tunnel(id, input).await;
                self.reply(reply, result);
            }
            Request::Copy(reference, reply) => {
                let result = self.copy(reference).await;
                self.reply(reply, result);
            }
            Request::Delete(reference, reply) => {
                let result = self.delete(reference).await;
                self.reply(reply, result);
            }
            Request::Start(kind, cancel, output, reply) => self.start(kind, cancel, output, reply),
            Request::Options(options, reply) => {
                let result = self.options(options).await;
                self.reply(reply, result);
            }
            Request::Verify(request, fingerprint, cancel, reply) => {
                self.verify(request, fingerprint, cancel, reply)
            }
            Request::Password(target, owner, username, cancel, reply) => {
                if let Err(error) = self.admit().and_then(|()| {
                    if cancel.is_cancelled() {
                        Err(cancelled())
                    } else {
                        Ok(())
                    }
                }) {
                    let _ = reply.send(Err(error));
                    return;
                }
                self.prompt(
                    target,
                    owner,
                    TerminalPromptKind::Password { username },
                    cancel,
                    PromptReply::Password(reply),
                );
            }
            Request::ConfirmHost(id, accept, reply) => {
                let result = self.confirm_host(id, accept).await;
                self.reply(reply, result);
            }
            Request::AnswerPassword(id, password, remember, reply) => {
                let result = if std::str::from_utf8(password.expose()).is_err() {
                    Err(Error::Configuration)
                } else {
                    self.answer_password(id, password, remember)
                };
                self.reply(reply, result);
            }
            Request::CancelPrompt(id, reply) => {
                self.cancel(id);
                self.reply(reply, Ok(()));
            }
            Request::SavePassword(id, revision, password, cancel, reply) => {
                let result = self.save_password(id, revision, password, cancel).await;
                self.reply(reply, result);
            }
            Request::ReplaceHost(id, expected, key, reply) => {
                let result = self.replace_host(id, expected, key).await;
                self.reply(reply, result);
            }
            Request::Issue(issue, reply) => {
                self.issue = Some(issue);
                self.reply(reply, Ok(()));
            }
            Request::Closing(reply) => {
                self.closing = true;
                self.cancel_all();
                self.reply(reply, Ok(()));
            }
            Request::Finish => unreachable!(),
        }
    }
}
