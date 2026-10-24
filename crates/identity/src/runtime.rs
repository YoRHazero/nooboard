use crate::backend::{self, Material};
use crate::{IdentityStatus, ServiceState, *};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{mpsc, oneshot, watch};

pub(crate) enum Request {
    Sign {
        scheme: SignatureScheme,
        message: Vec<u8>,
        reply: oneshot::Sender<Result<Vec<u8>>>,
    },
    SignBlocking {
        scheme: SignatureScheme,
        message: Vec<u8>,
        reply: std::sync::mpsc::SyncSender<Result<Vec<u8>>>,
    },
    Seal {
        context: SecretContext,
        plaintext: SecretBytes,
        reply: oneshot::Sender<Result<SealedSecret>>,
    },
    Open {
        context: SecretContext,
        sealed: SealedSecret,
        reply: oneshot::Sender<Result<SecretBytes>>,
    },
    Wake,
}
pub(crate) struct Worker {
    pub config: IdentityConfig,
    pub options: IdentityOptions,
    pub requests: mpsc::Receiver<Request>,
    pub stopping: Arc<AtomicBool>,
    pub ready: oneshot::Sender<Result<watch::Receiver<IdentityStatus>>>,
}
impl Worker {
    pub fn run(mut self) -> Result<()> {
        let material = match backend::load(self.config, &self.options) {
            Ok(material) => material,
            Err(error) => {
                let _ = self.ready.send(Err(error));
                return Err(error);
            }
        };
        let public = match material.public() {
            Ok(public) => public,
            Err(error) => {
                let _ = self.ready.send(Err(error));
                return Err(error);
            }
        };
        let (status, receiver) = watch::channel(IdentityStatus {
            state: ServiceState::Running,
            identity: public,
        });
        if self.ready.send(Ok(receiver)).is_err() {
            return Ok(());
        }
        struct Finished(watch::Sender<IdentityStatus>);
        impl Drop for Finished {
            fn drop(&mut self) {
                self.0.send_modify(|s| {
                    s.state = if std::thread::panicking() {
                        ServiceState::Failed
                    } else {
                        ServiceState::Stopped
                    }
                });
            }
        }
        let _finished = Finished(status);
        while let Some(request) = self.requests.blocking_recv() {
            if self.stopping.load(Ordering::Acquire) {
                break;
            }
            dispatch(&material, request);
        }
        self.requests.close();
        // Drop queued plaintext and reply senders before publishing the final state.
        while self.requests.try_recv().is_ok() {}
        drop(material); // Erase owned key material before announcing final worker completion.
        Ok(())
    }
}
fn dispatch(material: &Material, request: Request) {
    match request {
        Request::Sign {
            scheme,
            message,
            reply,
        } => {
            if !reply.is_closed() {
                let _ = reply.send(material.sign(scheme, &message));
            }
        }
        Request::SignBlocking {
            scheme,
            message,
            reply,
        } => {
            let _ = reply.send(material.sign(scheme, &message));
        }
        Request::Seal {
            context,
            plaintext,
            reply,
        } => {
            if !reply.is_closed() {
                let _ = reply.send(material.seal(context, plaintext));
            }
        }
        Request::Open {
            context,
            sealed,
            reply,
        } => {
            if !reply.is_closed() {
                let _ = reply.send(material.open(context, sealed));
            }
        }
        Request::Wake => {}
    }
}
