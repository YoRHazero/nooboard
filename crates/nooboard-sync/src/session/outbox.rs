use std::collections::VecDeque;
use crate::protocol::{DataPacket, Packet};

pub const CONTROL_QUEUE_CAPACITY: usize = 64;
pub const DATA_QUEUE_CAPACITY: usize = 256;
pub const MESSAGE_QUEUE_CAPACITY: usize = 64;
const HIGH_PRIORITY_MAX: u8 = 10;

pub struct PacketOutbox {
    control_queue: VecDeque<Packet>,
    data_queue: VecDeque<Packet>,
    message_queue: VecDeque<Packet>,
    high_streak: u8,
}

impl PacketOutbox {
    pub fn new() -> Self {
        Self {
            control_queue: VecDeque::new(),
            data_queue: VecDeque::new(),
            message_queue: VecDeque::new(),
            high_streak: 0,
        }
    }

    pub fn queue_control(&mut self, packet: Packet) -> bool {
        if self.control_queue.len() >= CONTROL_QUEUE_CAPACITY {
            return false;
        }
        self.control_queue.push_back(packet);
        true
    }

    pub fn queue_data(&mut self, packet: Packet) -> Result<(), Packet> {
        let is_message = matches!(
            packet,
            Packet::Data(DataPacket::FileDecision { .. }) |
            Packet::Data(DataPacket::FileCancel { .. })
        );
        if is_message {
            if self.message_queue.len() >= MESSAGE_QUEUE_CAPACITY {
                return Err(packet);
            }
            self.message_queue.push_back(packet);
            return Ok(());
        } else {
            if self.data_queue.len() >= DATA_QUEUE_CAPACITY {
                return Err(packet);
            }
            self.data_queue.push_back(packet);
            return Ok(());
        }
    }

    pub fn has_pending(&self) -> bool {
        !self.control_queue.is_empty() || !self.data_queue.is_empty() || !self.message_queue.is_empty()
    }

    pub fn remaining_data_capacity(&self) -> usize {
        DATA_QUEUE_CAPACITY.saturating_sub(self.data_queue.len())
    }

    pub fn pop_next(&mut self) -> Option<Packet> {
        let has_data = !self.data_queue.is_empty();
        let high_priority_saturated = self.high_streak >= HIGH_PRIORITY_MAX;

        if !high_priority_saturated || !has_data {
            if let Some(packet) = self.control_queue.pop_front()
                .or_else(|| self.message_queue.pop_front())
            {
                self.high_streak = self.high_streak.saturating_add(1);
                return Some(packet);
            }

        }
        if let Some(packet) = self.data_queue.pop_front() {
            self.high_streak = 0;
            return Some(packet);
        }
        None
    }
}
