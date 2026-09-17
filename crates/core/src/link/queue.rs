//! A bounded per-peer outbox. Only adjacent, unsent automatic updates coalesce.
use nooboard_network::{Message, MessageId};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use tokio::sync::Notify;

#[derive(Clone)]
pub(crate) struct Outbox(Arc<Inner>);
struct Inner {
    queue: Mutex<Queue>,
    ready: Notify,
    space: Notify,
}
#[derive(Default)]
struct Queue {
    control: VecDeque<Message>,
    text: VecDeque<Job>,
    bulk: VecDeque<Message>,
}
pub(crate) struct Job {
    pub message: Message,
    pub automatic: bool,
}
impl Job {
    pub fn id(&self) -> Option<&MessageId> {
        match &self.message {
            Message::Text { id, .. } => Some(id),
            _ => None,
        }
    }
}
impl Outbox {
    pub fn new() -> Self {
        Self(Arc::new(Inner {
            queue: Mutex::new(Queue::default()),
            ready: Notify::new(),
            space: Notify::new(),
        }))
    }
    pub fn text(&self, job: Job) -> Result<Option<MessageId>, ()> {
        let mut q = self.0.queue.lock().expect("outbox lock");
        let replaced = if job.automatic && q.text.back().is_some_and(|j| j.automatic) {
            q.text.pop_back().and_then(|j| j.id().cloned())
        } else {
            None
        };
        if q.text.len() >= 32 {
            return Err(());
        }
        q.text.push_back(job);
        self.0.ready.notify_one();
        Ok(replaced)
    }
    pub fn control(&self, message: Message) -> Result<(), ()> {
        let mut q = self.0.queue.lock().expect("outbox lock");
        if q.control.len() >= 64 {
            return Err(());
        }
        q.control.push_back(message);
        self.0.ready.notify_one();
        Ok(())
    }
    pub async fn bulk(&self, message: Message) {
        loop {
            let notified = self.0.space.notified();
            {
                let mut q = self.0.queue.lock().expect("outbox lock");
                if q.bulk.len() < 2 {
                    q.bulk.push_back(message);
                    self.0.ready.notify_one();
                    return;
                }
            }
            notified.await;
        }
    }
    pub fn cancel_bulk(&self, id: &MessageId) {
        let mut q = self.0.queue.lock().expect("outbox lock");
        q.bulk
            .retain(|m| !matches!(m, Message::Chunk { id: other, .. } if other == id));
        self.0.space.notify_one();
    }
    pub fn cancel_text(&self, automatic_only: bool) -> Vec<MessageId> {
        let mut q = self.0.queue.lock().expect("outbox lock");
        let mut ids = Vec::new();
        q.text.retain(|j| {
            if !automatic_only || j.automatic {
                if let Some(id) = j.id() {
                    ids.push(id.clone());
                }
                false
            } else {
                true
            }
        });
        ids
    }
    pub async fn next(&self) -> Job {
        loop {
            let notified = self.0.ready.notified();
            {
                let mut q = self.0.queue.lock().expect("outbox lock");
                if let Some(message) = q.control.pop_front() {
                    return Job {
                        message,
                        automatic: false,
                    };
                }
                if let Some(job) = q.text.pop_front() {
                    return job;
                }
                if let Some(message) = q.bulk.pop_front() {
                    self.0.space.notify_one();
                    return Job {
                        message,
                        automatic: false,
                    };
                }
            }
            notified.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn job(sequence: u64, automatic: bool) -> Job {
        Job {
            automatic,
            message: Message::Text {
                id: MessageId {
                    session: "a".repeat(32),
                    sequence,
                },
                target_epoch: 1,
                text: sequence.to_string(),
            },
        }
    }
    #[tokio::test]
    async fn coalescing_never_reorders_across_manual_sends_or_changes_in_flight_text() {
        let q = Outbox::new();
        q.text(job(1, true)).unwrap();
        let in_flight = q.next().await;
        q.text(job(2, true)).unwrap();
        assert_eq!(q.text(job(3, true)).unwrap().unwrap().sequence, 2);
        q.text(job(4, false)).unwrap();
        q.text(job(5, true)).unwrap();
        assert_eq!(q.text(job(6, true)).unwrap().unwrap().sequence, 5);
        assert_eq!(in_flight.id().unwrap().sequence, 1);
        for expected in [3, 4, 6] {
            assert_eq!(q.next().await.id().unwrap().sequence, expected);
        }
    }
    #[tokio::test]
    async fn saturated_peer_does_not_fill_another_queue_and_controls_still_pass() {
        let slow = Outbox::new();
        let healthy = Outbox::new();
        for sequence in 1..=32 {
            slow.text(job(sequence, false)).unwrap();
        }
        assert!(slow.text(job(33, false)).is_err());
        healthy.text(job(34, false)).unwrap();
        slow.control(Message::Pong).unwrap();
        assert_eq!(slow.next().await.message, Message::Pong);
        assert_eq!(healthy.next().await.id().unwrap().sequence, 34);
        assert_eq!(slow.cancel_text(false).len(), 32);
    }
    #[test]
    fn disabling_automatic_only_cancels_automatic_jobs() {
        let q = Outbox::new();
        q.text(job(1, true)).unwrap();
        q.text(job(2, false)).unwrap();
        q.text(job(3, true)).unwrap();
        assert_eq!(
            q.cancel_text(true)
                .iter()
                .map(|id| id.sequence)
                .collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert_eq!(q.cancel_text(false)[0].sequence, 2);
    }
}
