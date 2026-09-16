//! Connection lifecycle only. Clipboard, pairing persistence and business routing live in core.
mod listener;
pub(crate) mod queue;
mod session;
pub(crate) use listener::{Dial, Listener};
use nooboard_network::{Connection, Message, MessageId};
pub(crate) use session::Session;

pub(crate) enum LinkEvent {
    Connected {
        connection: Connection,
        initiated: bool,
        dial_generation: Option<u64>,
    },
    Message {
        peer: String,
        generation: u64,
        message: Message,
    },
    Started {
        peer: String,
        generation: u64,
        id: MessageId,
    },
    Written {
        peer: String,
        generation: u64,
        id: MessageId,
    },
    Offline {
        peer: String,
        generation: u64,
    },
    Fault {
        peer: Option<String>,
        message: String,
    },
}
