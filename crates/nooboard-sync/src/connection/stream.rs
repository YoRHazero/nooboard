use std::collections::VecDeque;

use tokio::net::TcpStream;
use tokio_rustls::TlsStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::protocol::Packet;
use crate::transport::{recv_packet, send_packet};

use super::ConnectionResult;

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

    pub fn queue_control(&mut self, packet: Packet) {
        self.control_queue.push_back(packet);
    }

    pub fn queue_data(&mut self, packet: Packet) {
        self.data_queue.push_back(packet);
    }

    pub async fn flush_one(&mut self) -> ConnectionResult<bool> {
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

    pub async fn recv(&mut self) -> ConnectionResult<Option<Packet>> {
        Ok(recv_packet(&mut self.framed).await?)
    }
}
