use crate::{Payload, Result, Snapshot};
use tokio::sync::oneshot;

pub(crate) enum Command {
    Read,
    Write(Payload),
}
pub(crate) struct Request {
    pub command: Command,
    pub reply: oneshot::Sender<Result<Snapshot>>,
}
