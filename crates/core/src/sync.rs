use crate::{Error, Result};

/// Lamport order is scoped to one authenticated connection; no wall clock comparison.
#[derive(Default)]
pub(crate) struct Order {
    clock: u64,
    applied: Option<(u64, String)>,
    seen_peer: u64,
}
impl Order {
    pub fn local(&mut self, identity: &str) -> Result<u64> {
        self.clock = self.clock.checked_add(1).ok_or(Error::Configuration)?;
        self.applied = Some((self.clock, identity.into()));
        Ok(self.clock)
    }
    pub fn should_apply(&mut self, sequence: u64, identity: &str) -> bool {
        self.clock = self.clock.max(sequence);
        if sequence == 0 || sequence <= self.seen_peer {
            return false;
        }
        self.seen_peer = sequence;
        let candidate = (sequence, identity.to_owned());
        self.applied
            .as_ref()
            .is_none_or(|current| candidate > *current)
    }
    pub fn applied(&mut self, sequence: u64, identity: &str) {
        self.applied = Some((sequence, identity.into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concurrent_copies_converge_and_duplicates_do_not_reapply() {
        let mut a = Order::default();
        let mut b = Order::default();
        assert_eq!(a.local("a").unwrap(), 1);
        assert_eq!(b.local("b").unwrap(), 1);
        assert!(a.should_apply(1, "b"));
        a.applied(1, "b");
        assert!(!b.should_apply(1, "a"));
        assert!(!a.should_apply(1, "b"));
        assert_eq!(a.local("a").unwrap(), 2);
        assert!(b.should_apply(2, "a"));
    }
}
