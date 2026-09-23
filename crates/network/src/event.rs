use crate::{
    identity::{PublicIdentity, TrustedPeer},
    pairing::PairingId,
    transfer::{ContentKind, FileEntry, IncomingId, ReceivedContent, TransferId},
};

/// Single-consumer application work. Progress belongs in the status snapshots.
/// Dropping or failing to drain the inbox never silently applies incoming content.
#[derive(Debug)]
pub enum NetworkEvent {
    PairingOffered {
        id: PairingId,
        peer: PublicIdentity,
    },
    /// Persist this public record, then call `complete_pairing(id, saved)`.
    PairingVerified {
        id: PairingId,
        peer: TrustedPeer,
    },
    IncomingOffer {
        id: IncomingId,
        transfer_id: TransferId,
        peer: String,
        kind: ContentKind,
        files: Vec<FileEntry>,
    },
    /// Report the caller's actual application result with `complete_incoming`.
    ContentReady {
        id: IncomingId,
        transfer_id: TransferId,
        peer: String,
        content: ReceivedContent,
    },
}
