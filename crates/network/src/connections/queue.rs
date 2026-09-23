//! One bounded outbox per peer. Backpressure and closure never depend on the supervisor queue.
use crate::{
    error::{Failure, InternalResult as Result},
    transfer::{
        QueuePolicy,
        protocol::{Message, MessageId},
    },
};
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
    closed: Notify,
}
#[derive(Default)]
struct Queue {
    control: VecDeque<Message>,
    text: VecDeque<Job>,
    bulk: VecDeque<Message>,
    closed: bool,
    priority_run: u8,
    control_run: u8,
}
pub(crate) struct Job {
    pub message: Message,
    pub policy: QueuePolicy,
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
            closed: Notify::new(),
        }))
    }
    pub fn text(&self, message: Message, policy: QueuePolicy) -> Result<Option<MessageId>> {
        let mut q = self.0.queue.lock().unwrap();
        if q.closed {
            return Err(Failure::Disconnected);
        }
        let replaced = if matches!(&policy, QueuePolicy::ReplaceTail(_))
            && q.text.back().is_some_and(|j| j.policy == policy)
        {
            q.text.pop_back().and_then(|j| j.id().cloned())
        } else {
            None
        };
        if q.text.len() >= 32 {
            return Err(Failure::Busy);
        }
        q.text.push_back(Job { message, policy });
        self.0.ready.notify_one();
        Ok(replaced)
    }
    pub fn control(&self, message: Message) -> Result<()> {
        let mut q = self.0.queue.lock().unwrap();
        if q.closed {
            return Err(Failure::Disconnected);
        }
        if q.control.len() >= 64 {
            return Err(Failure::Busy);
        }
        q.control.push_back(message);
        self.0.ready.notify_one();
        Ok(())
    }
    pub async fn bulk(&self, message: Message) -> Result<()> {
        loop {
            let notified = self.0.space.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            {
                let mut q = self.0.queue.lock().unwrap();
                if q.closed {
                    return Err(Failure::Disconnected);
                }
                if q.bulk.len() < 2 {
                    q.bulk.push_back(message);
                    self.0.ready.notify_one();
                    return Ok(());
                }
            }
            notified.await;
        }
    }
    pub fn cancel(&self, id: &MessageId) -> bool {
        let mut q = self.0.queue.lock().unwrap();
        let before = q.text.len();
        q.text.retain(|j| j.id() != Some(id));
        q.bulk
            .retain(|m| !matches!(m, Message::Chunk { id: other, .. } if other == id));
        self.0.space.notify_waiters();
        before != q.text.len()
    }
    pub async fn next(&self) -> Option<Job> {
        loop {
            let ready = self.0.ready.notified();
            tokio::pin!(ready);
            ready.as_mut().enable();
            {
                let mut q = self.0.queue.lock().unwrap();
                if q.closed {
                    return None;
                }
                // Reserve a turn for bulk after a bounded control/text burst.
                if q.priority_run >= 8
                    && let Some(message) = q.bulk.pop_front()
                {
                    q.priority_run = 0;
                    self.0.space.notify_waiters();
                    return Some(Job {
                        message,
                        policy: QueuePolicy::Append,
                    });
                }
                if q.control_run >= 8
                    && let Some(job) = q.text.pop_front()
                {
                    q.control_run = 0;
                    q.priority_run = q.priority_run.saturating_add(1);
                    return Some(job);
                }
                if let Some(message) = q.control.pop_front() {
                    q.control_run = q.control_run.saturating_add(1);
                    q.priority_run = q.priority_run.saturating_add(1);
                    return Some(Job {
                        message,
                        policy: QueuePolicy::Append,
                    });
                }
                if let Some(job) = q.text.pop_front() {
                    q.control_run = 0;
                    q.priority_run = q.priority_run.saturating_add(1);
                    return Some(job);
                }
                if let Some(message) = q.bulk.pop_front() {
                    q.priority_run = 0;
                    self.0.space.notify_waiters();
                    return Some(Job {
                        message,
                        policy: QueuePolicy::Append,
                    });
                }
            }
            ready.await;
        }
    }
    pub fn close(&self) {
        let mut q = self.0.queue.lock().unwrap();
        q.closed = true;
        q.control.clear();
        q.text.clear();
        q.bulk.clear();
        self.0.ready.notify_waiters();
        self.0.space.notify_waiters();
        self.0.closed.notify_waiters();
    }
    pub fn is_closed(&self) -> bool {
        self.0.queue.lock().unwrap().closed
    }
    pub async fn closed(&self) {
        loop {
            let notified = self.0.closed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.is_closed() {
                return;
            }
            notified.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(n: u64) -> MessageId {
        MessageId {
            session: "a".repeat(32),
            sequence: n,
        }
    }
    fn text(n: u64) -> Message {
        Message::Text {
            id: id(n),
            target_epoch: 1,
            text: n.to_string(),
        }
    }
    fn chunk(n: u64) -> Message {
        Message::Chunk {
            id: id(n),
            file: 0,
            offset: 0,
            bytes: vec![1],
        }
    }
    #[tokio::test]
    async fn replacement_only_affects_the_adjacent_unstarted_tail() {
        let queue = Outbox::new();
        let replace = QueuePolicy::ReplaceTail("clipboard".into());
        queue.text(text(1), replace.clone()).unwrap();
        let active = queue.next().await.unwrap();
        queue.text(text(2), replace.clone()).unwrap();
        assert_eq!(queue.text(text(3), replace.clone()).unwrap(), Some(id(2)));
        queue.text(text(4), QueuePolicy::Append).unwrap();
        queue.text(text(5), replace.clone()).unwrap();
        assert_eq!(queue.text(text(6), replace).unwrap(), Some(id(5)));
        assert_eq!(active.id(), Some(&id(1)));
        for n in [3, 4, 6] {
            assert_eq!(queue.next().await.unwrap().id(), Some(&id(n)));
        }
    }
    #[tokio::test]
    async fn closure_wakes_a_saturated_bulk_producer_and_an_empty_consumer() {
        let queue = Outbox::new();
        queue.bulk(chunk(1)).await.unwrap();
        queue.bulk(chunk(2)).await.unwrap();
        let copy = queue.clone();
        let producer = tokio::spawn(async move { copy.bulk(chunk(3)).await });
        tokio::task::yield_now().await;
        queue.close();
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), producer)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        let queue = Outbox::new();
        let copy = queue.clone();
        let consumer = tokio::spawn(async move { copy.next().await });
        tokio::task::yield_now().await;
        queue.close();
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), consumer)
                .await
                .unwrap()
                .unwrap()
                .is_none()
        );
    }
    #[tokio::test]
    async fn priority_traffic_cannot_starve_text_or_bulk() {
        let queue = Outbox::new();
        for _ in 0..32 {
            queue.control(Message::Pong).unwrap();
        }
        queue.text(text(1), QueuePolicy::Append).unwrap();
        queue.bulk(chunk(2)).await.unwrap();
        let mut text_seen = false;
        let mut bulk_seen = false;
        for _ in 0..10 {
            match queue.next().await.unwrap().message {
                Message::Text { .. } => text_seen = true,
                Message::Chunk { .. } => bulk_seen = true,
                _ => {}
            }
        }
        assert!(text_seen && bulk_seen);
    }
}
