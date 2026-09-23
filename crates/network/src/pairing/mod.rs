//! One-time-code pairing is separate from the pinned mutual-TLS clipboard transport.
mod model;
pub(crate) mod runtime;
pub use model::{PairingId, PairingStatus, Stage};
mod crypto;
mod endpoint;
mod session;
mod wire;
use endpoint::Endpoint;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot, watch};
use zeroize::Zeroizing;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contact {
    pub device_name: String,
    pub certificate: Vec<u8>,
    pub sync_port: u16,
}
impl Contact {
    pub fn validate(&self) -> Result<String> {
        if !crate::identity::valid_device_name(&self.device_name)
            || self.certificate.len() > 8192
            || self.sync_port == 0
        {
            return Err(Error::Protocol);
        }
        crate::identity::material::noob_id(&self.certificate).map_err(|_| Error::Protocol)
    }
}
#[derive(Clone)]
pub struct Control {
    action: mpsc::Sender<Action>,
    cancel: watch::Sender<bool>,
}
impl Control {
    pub fn accept(&self) -> Result<()> {
        self.action
            .try_send(Action::Accept)
            .map_err(|_| Error::Busy)
    }
    pub fn code(&self, code: String) -> Result<()> {
        if code.len() != 8 || !code.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Error::Code);
        }
        self.action
            .try_send(Action::Code(Zeroizing::new(code)))
            .map_err(|_| Error::Busy)
    }
    pub fn cancel(&self) {
        self.cancel.send_replace(true);
    }
}
enum Action {
    Accept,
    Code(Zeroizing<String>),
}
pub enum Event {
    Offered {
        id: String,
        peer: Contact,
        control: Control,
        incoming: bool,
    },
    Progress {
        id: String,
        stage: Stage,
        code: Option<String>,
        attempts_left: u8,
    },
    Verified {
        id: String,
        peer: Contact,
        address: String,
        saved: oneshot::Sender<bool>,
    },
    Completed {
        id: String,
    },
    Failed {
        id: String,
        error: Error,
    },
}
#[derive(Clone, Debug, Serialize, thiserror::Error)]
pub enum Error {
    #[error("配对信息无效或协议不兼容。")]
    Protocol,
    #[error("配对码不正确。")]
    Code,
    #[error("配对已过期，请重新发起。")]
    Timeout,
    #[error("对方拒绝了配对，或正在处理另一个请求。")]
    Rejected,
    #[error("配对已取消。")]
    Cancelled,
    #[error("配对尝试次数已用完，请重新发起。")]
    Attempts,
    #[error("配对连接已断开；若一方已保存设备，可重新发起配对。")]
    Disconnected,
    #[error("暂时无法处理配对请求。")]
    Busy,
    #[error("设备配对信息未能保存。")]
    Storage,
    #[error("无法连接配对地址，请检查地址、端口与对方是否运行。")]
    Connect,
}
pub type Result<T> = std::result::Result<T, Error>;
fn control() -> (Control, mpsc::Receiver<Action>, watch::Receiver<bool>) {
    let (action, actions) = mpsc::channel(1);
    let (cancel, cancelled) = watch::channel(false);
    (Control { action, cancel }, actions, cancelled)
}
fn random_code() -> Result<Zeroizing<String>> {
    // Rejection sampling avoids modulo bias; the code never appears on the wire.
    loop {
        let nonce = crate::identity::material::new_session_id().map_err(|_| Error::Protocol)?;
        let value = u32::from_str_radix(&nonce[..8], 16).map_err(|_| Error::Protocol)?;
        if value < 4_200_000_000 {
            return Ok(Zeroizing::new(format!("{:08}", value % 100_000_000)));
        }
    }
}
