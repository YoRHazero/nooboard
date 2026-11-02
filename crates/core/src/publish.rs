//! The single publication rule for feature state watches: a notification means the value changed.
//!
//! Each feature runtime owns its state and projects it in one `publish()`; that projection
//! goes through this trait. The application supervisor only forwards, so it needs no comparison.
use tokio::sync::watch;

/// A view that numbers its own changes.
pub(crate) trait Revised {
    fn revision_mut(&mut self) -> &mut u64;
}

pub(crate) trait Publish<T> {
    /// Replace the published value, notifying observers only if it differs.
    fn publish(&self, value: T) -> bool;
    /// Like `publish`, for a view that carries a revision: the revision advances
    /// only together with a change, so an unchanged view keeps its number.
    fn publish_revised(&self, value: T) -> bool
    where
        T: Revised;
}
impl<T: PartialEq> Publish<T> for watch::Sender<T> {
    fn publish(&self, value: T) -> bool {
        self.send_if_modified(|current| {
            if *current == value {
                return false;
            }
            *current = value;
            true
        })
    }
    fn publish_revised(&self, mut value: T) -> bool
    where
        T: Revised,
    {
        self.send_if_modified(|current| {
            let revision = *current.revision_mut();
            *value.revision_mut() = revision;
            if *current == value {
                return false;
            }
            *value.revision_mut() = revision.wrapping_add(1);
            *current = value;
            true
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_different_value_notifies() {
        let (sender, mut receiver) = watch::channel(1);
        assert!(!sender.publish(1));
        assert!(!receiver.has_changed().unwrap());
        assert!(sender.publish(2));
        assert!(receiver.has_changed().unwrap());
        assert_eq!(*receiver.borrow_and_update(), 2);
    }

    #[derive(PartialEq)]
    struct View {
        revision: u64,
        value: &'static str,
    }
    impl Revised for View {
        fn revision_mut(&mut self) -> &mut u64 {
            &mut self.revision
        }
    }
    #[test]
    fn a_revision_advances_only_with_a_change() {
        let view = |value| View { revision: 0, value };
        let (sender, mut receiver) = watch::channel(view("a"));
        assert!(!sender.publish_revised(view("a")));
        assert!(!receiver.has_changed().unwrap());
        assert_eq!(receiver.borrow().revision, 0);
        assert!(sender.publish_revised(view("b")));
        assert_eq!(receiver.borrow_and_update().revision, 1);
        assert!(!sender.publish_revised(view("b")));
        assert_eq!(receiver.borrow().revision, 1);
    }
}
