#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DedupeDecision {
    ConnectOut,
    WaitInbound,
    RejectConflict,
}

pub(super) fn dedupe_decision(local_node_id: &str, peer_node_id: &str) -> DedupeDecision {
    if local_node_id == peer_node_id {
        return DedupeDecision::RejectConflict;
    }

    if local_node_id < peer_node_id {
        DedupeDecision::ConnectOut
    } else {
        DedupeDecision::WaitInbound
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smaller_node_id_must_initiate_connection() {
        assert_eq!(dedupe_decision("a", "b"), DedupeDecision::ConnectOut);
        assert_eq!(dedupe_decision("z", "b"), DedupeDecision::WaitInbound);
        assert_eq!(
            dedupe_decision("same", "same"),
            DedupeDecision::RejectConflict
        );
    }
}
