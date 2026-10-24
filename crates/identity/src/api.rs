//! Public capabilities never export application identity private material or derived keys.
use crate::runtime::{Request, Worker};
pub use crate::{
    error::{Error, Result},
    model::*,
    options::IdentityOptions,
    secret::SecretBytes,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};

pub struct IdentityService {
    handle: Identity,
    task: Option<JoinHandle<Result<()>>>,
    failure: Option<Error>,
}
#[derive(Clone)]
pub struct Identity {
    requests: mpsc::Sender<Request>,
    stopping: Arc<AtomicBool>,
    status: watch::Receiver<IdentityStatus>,
    options: Arc<IdentityOptions>,
}
impl IdentityService {
    pub async fn start(
        config: IdentityConfig,
        options: IdentityOptions,
    ) -> Result<(Self, Identity)> {
        config.validate()?;
        options.validate()?;
        let runtime = tokio::runtime::Handle::try_current().map_err(|_| Error::InvalidInput)?;
        let (requests, receiver) = mpsc::channel(options.queue_capacity);
        let stopping = Arc::new(AtomicBool::new(false));
        let (ready, started) = oneshot::channel();
        let worker = Worker {
            config,
            options: options.clone(),
            requests: receiver,
            stopping: stopping.clone(),
            ready,
        };
        // A dedicated blocking worker also serves synchronous rustls signing callbacks.
        // It never waits for a task on the runtime that is currently requesting a signature.
        let task = runtime.spawn_blocking(move || worker.run());
        let status = match started.await {
            Ok(Ok(status)) => status,
            Ok(Err(error)) => {
                let _ = task.await;
                return Err(error);
            }
            Err(_) => {
                let _ = task.await;
                return Err(Error::Internal);
            }
        };
        let handle = Identity {
            requests,
            stopping,
            status,
            options: Arc::new(options),
        };
        Ok((
            Self {
                handle: handle.clone(),
                task: Some(task),
                failure: None,
            },
            handle,
        ))
    }
    pub fn handle(&self) -> Identity {
        self.handle.clone()
    }
    pub async fn shutdown(&mut self) -> Result<()> {
        self.stop();
        if let Some(error) = self.failure {
            return Err(error);
        }
        let Some(task) = self.task.as_mut() else {
            return Ok(());
        };
        match tokio::time::timeout(self.handle.options.shutdown_timeout, task).await {
            Ok(result) => {
                self.task.take();
                let result = result.map_err(|_| Error::Internal).and_then(|r| r);
                self.failure = result.as_ref().err().copied();
                result
            }
            Err(_) => Err(Error::Timeout),
        }
    }
    fn stop(&self) {
        self.handle.stopping.store(true, Ordering::Release);
        let _ = self.handle.requests.try_send(Request::Wake);
    }
}
impl Drop for IdentityService {
    fn drop(&mut self) {
        self.stop();
    }
}
impl Identity {
    pub fn public_identity(&self) -> PublicIdentity {
        self.status.borrow().identity.clone()
    }
    pub fn status(&self) -> IdentityStatus {
        let mut status = self.status.borrow().clone();
        if status.state == ServiceState::Running && self.stopping.load(Ordering::Acquire) {
            status.state = ServiceState::Stopping;
        }
        status
    }
    fn admit(&self) -> Result<()> {
        if self.stopping.load(Ordering::Acquire) || self.requests.is_closed() {
            Err(Error::Stopped)
        } else {
            Ok(())
        }
    }
    async fn request<T>(
        &self,
        make: impl FnOnce(oneshot::Sender<Result<T>>) -> Request,
    ) -> Result<T> {
        self.admit()?;
        let (reply, receive) = oneshot::channel();
        self.requests
            .send(make(reply))
            .await
            .map_err(|_| Error::Stopped)?;
        receive.await.map_err(|_| Error::Stopped)?
    }
    pub async fn sign(&self, scheme: SignatureScheme, message: Vec<u8>) -> Result<Vec<u8>> {
        if message.len() > self.options.max_signing_bytes {
            return Err(Error::InvalidInput);
        }
        self.request(|reply| Request::Sign {
            scheme,
            message,
            reply,
        })
        .await
    }
    /// For synchronous TLS signing APIs only. Does not call Tokio blocking_send/blocking_recv.
    /// The crypto worker is independent, so this also works on a current-thread runtime.
    pub fn sign_blocking(&self, scheme: SignatureScheme, message: &[u8]) -> Result<Vec<u8>> {
        self.admit()?;
        if message.len() > self.options.max_signing_bytes {
            return Err(Error::InvalidInput);
        }
        let (reply, receive) = std::sync::mpsc::sync_channel(1);
        self.requests
            .try_send(Request::SignBlocking {
                scheme,
                message: message.to_vec(),
                reply,
            })
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => Error::Busy,
                mpsc::error::TrySendError::Closed(_) => Error::Stopped,
            })?;
        receive
            .recv_timeout(self.options.signing_timeout)
            .map_err(|e| match e {
                std::sync::mpsc::RecvTimeoutError::Timeout => Error::Timeout,
                std::sync::mpsc::RecvTimeoutError::Disconnected => Error::Stopped,
            })?
    }
    pub async fn seal(
        &self,
        context: SecretContext,
        plaintext: SecretBytes,
    ) -> Result<SealedSecret> {
        self.options.validate_context(&context)?;
        if plaintext.expose_secret().len() > self.options.max_secret_bytes {
            return Err(Error::InvalidInput);
        }
        self.request(|reply| Request::Seal {
            context,
            plaintext,
            reply,
        })
        .await
    }
    pub async fn open(&self, context: SecretContext, sealed: SealedSecret) -> Result<SecretBytes> {
        self.options.validate_context(&context)?;
        if sealed.version != 1 {
            return Err(Error::UnsupportedVersion);
        }
        if sealed.ciphertext.len() < 16
            || sealed.ciphertext.len() > self.options.max_secret_bytes + 16
        {
            return Err(Error::InvalidInput);
        }
        self.request(|reply| Request::Open {
            context,
            sealed,
            reply,
        })
        .await
    }
}
