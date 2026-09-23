use crate::{Error, Result};
use tokio::sync::{mpsc, oneshot, watch};
pub(crate) type Reply<T> = oneshot::Sender<Result<T>>;
pub(crate) async fn request<T, Q>(
    sender: &mpsc::Sender<Q>,
    stopped: &watch::Receiver<bool>,
    make: impl FnOnce(Reply<T>) -> Q,
) -> Result<T> {
    let mut stop = stopped.clone();
    if *stop.borrow() {
        return Err(Error::Stopped);
    }
    let (reply, response) = oneshot::channel();
    tokio::select! { biased; _=stop.changed()=>return Err(Error::Stopped), sent=sender.send(make(reply))=>sent.map_err(|_|Error::Stopped)?, }
    response.await.map_err(|_| Error::Stopped)?
}
