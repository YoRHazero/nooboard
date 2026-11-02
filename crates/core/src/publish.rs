//! The single publication rule for feature state watches: a notification means the value changed.
//!
//! Each feature runtime owns its state and projects it in one `publish()`; that projection
//! goes through this trait. The application supervisor only forwards, so it needs no comparison.
use tokio::sync::watch;

pub(crate) trait Publish<T> {
    /// Replace the published value, notifying observers only if it differs.
    fn publish(&self, value: T) -> bool;
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
}
