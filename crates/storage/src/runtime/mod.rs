pub(crate) mod request;
#[cfg(test)]
mod tests;
use crate::{Result, backend::contract::Backend};
use request::Request;
use tokio::sync::{mpsc, watch};

pub(crate) async fn run(
    backend: Box<dyn Backend>,
    mut requests: mpsc::Receiver<Request>,
    mut stop: watch::Receiver<bool>,
) -> Result<()> {
    loop {
        if *stop.borrow() {
            break;
        }
        let request = tokio::select! {
            biased;
            _ = stop.changed() => break,
            request = requests.recv() => match request { Some(request) => request, None => break },
        };
        // Stop has priority over work that has not begun. An active operation is
        // allowed to finish; shutting down must never pretend it was rolled back.
        if *stop.borrow() {
            request.reject();
            break;
        }
        request.execute(backend.as_ref()).await;
    }
    requests.close();
    while let Some(request) = requests.recv().await {
        request.reject();
    }
    backend.close().await
}
