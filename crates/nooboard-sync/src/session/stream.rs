use std::collections::VecDeque;

use tokio::net::TcpStream;
use tokio_rustls::TlsStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::protocol::{DataPacket, Packet};
use crate::transport::{recv_packet, send_packet};

use super::SessionResult;

pub const CONTROL_QUEUE_CAPACITY: usize = 64;
pub const DATA_QUEUE_CAPACITY: usize = 256;

pub struct PriorityPacketStream {
    framed: Framed<TlsStream<TcpStream>, LengthDelimitedCodec>,
    control_queue: VecDeque<Packet>,
    data_queue: VecDeque<Packet>,
}

impl PriorityPacketStream {
    pub fn new(framed: Framed<TlsStream<TcpStream>, LengthDelimitedCodec>) -> Self {
        Self {
            framed,
            control_queue: VecDeque::new(),
            data_queue: VecDeque::new(),
        }
    }

    pub fn try_queue_control(&mut self, packet: Packet) -> bool {
        if self.control_queue.len() >= CONTROL_QUEUE_CAPACITY {
            return false;
        }

        self.control_queue.push_back(packet);
        true
    }

    pub fn try_queue_data(&mut self, packet: Packet) -> bool {
        if self.data_queue.len() >= DATA_QUEUE_CAPACITY && !is_capacity_exempt(&packet) {
            return false;
        }

        self.data_queue.push_back(packet);
        true
    }

    pub fn has_pending(&self) -> bool {
        !(self.control_queue.is_empty() && self.data_queue.is_empty())
    }

    pub fn remaining_data_capacity(&self) -> usize {
        DATA_QUEUE_CAPACITY.saturating_sub(self.data_queue.len())
    }

    pub async fn flush_one(&mut self) -> SessionResult<bool> {
        if let Some(packet) = self.control_queue.pop_front() {
            send_packet(&mut self.framed, &packet).await?;
            return Ok(true);
        }

        if let Some(packet) = self.data_queue.pop_front() {
            send_packet(&mut self.framed, &packet).await?;
            return Ok(true);
        }

        Ok(false)
    }

    pub async fn recv(&mut self) -> SessionResult<Option<Packet>> {
        Ok(recv_packet(&mut self.framed).await?)
    }
}

fn is_capacity_exempt(packet: &Packet) -> bool {
    matches!(
        packet,
        Packet::Data(DataPacket::FileDecision { .. }) | Packet::Data(DataPacket::FileCancel { .. })
    )
}
