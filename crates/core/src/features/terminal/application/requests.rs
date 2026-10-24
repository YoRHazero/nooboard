use super::auth::PasswordAnswer;
use crate::{
    Result,
    contracts::terminal::*,
    features::terminal::domain::{config::*, inputs::*, views::*},
};
use tokio::sync::{mpsc, oneshot};
pub(crate) type Reply<T> = oneshot::Sender<Result<T>>;
pub(crate) enum Request {
    Ssh(
        Option<SshTargetConfigId>,
        SshTargetInput,
        Reply<SshTargetConfigId>,
    ),
    Command(
        Option<CommandConfigId>,
        CommandInput,
        Reply<CommandConfigId>,
    ),
    Terminal(
        Option<TerminalConfigId>,
        TerminalInput,
        Reply<TerminalConfigId>,
    ),
    Tunnel(Option<TunnelConfigId>, TunnelInput, Reply<TunnelConfigId>),
    Copy(TerminalConfigReference, Reply<TerminalConfigReference>),
    Delete(TerminalConfigReference, Reply<()>),
    Start(
        StartKind,
        CancellationToken,
        bool,
        Reply<(RunId, Option<TerminalOutputSubscription>)>,
    ),
    Options(TerminalServiceOptions, Reply<()>),
    Verify(
        SshHostVerificationRequest,
        String,
        CancellationToken,
        Reply<()>,
    ),
    Password(
        SshTargetConfigId,
        SshConnectionOwnerRunId,
        String,
        CancellationToken,
        Reply<PasswordAnswer>,
    ),
    ConfirmHost(TerminalPromptId, bool, Reply<()>),
    AnswerPassword(TerminalPromptId, SecretInput, bool, Reply<()>),
    CancelPrompt(TerminalPromptId, Reply<()>),
    SavePassword(
        SshTargetConfigId,
        u64,
        EncryptedSecret,
        CancellationToken,
        Reply<()>,
    ),
    ReplaceHost(
        SshTargetConfigId,
        Option<SshPublicKey>,
        SshPublicKey,
        Reply<()>,
    ),
    Issue(TerminalIssue, Reply<()>),
    Closing(Reply<()>),
    Finish,
}
#[derive(Clone)]
pub(crate) struct Coordinator(pub mpsc::Sender<Request>);
impl Coordinator {
    pub async fn call<T>(&self, make: impl FnOnce(Reply<T>) -> Request) -> Result<T> {
        let (reply, receive) = oneshot::channel();
        self.0
            .send(make(reply))
            .await
            .map_err(|_| crate::Error::Stopped)?;
        receive.await.map_err(|_| crate::Error::Stopped)?
    }
    pub async fn issue(&self, issue: TerminalIssue) {
        let _ = self.call(|reply| Request::Issue(issue, reply)).await;
    }
}
