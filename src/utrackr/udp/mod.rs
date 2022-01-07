//! The `utrackr::udp` modules implements BEP 15[^1].
//!
//! [^1]: [UDP Tracker Protocol for BitTorrent](https://www.bittorrent.org/beps/bep_0015.html)

use std::{io, sync::Arc};

use rand::random;
use tokio::net::UdpSocket;

use crate::core::{Tracker, UdpConfig};
use crate::udp::protocol::Secret;
use transaction::{Transaction};
use protocol::{MAX_PACKET_SIZE, MIN_PACKET_SIZE};

mod transaction;
mod protocol;

pub struct UdpTracker {
    socket: Arc<UdpSocket>,
    secret: Secret,
    tracker: Tracker,
}

impl UdpTracker {
    pub async fn bind(tracker: Tracker, config: UdpConfig) -> io::Result<Self> {
        let socket = UdpSocket::bind(config.bind.addrs()).await?;
        let addr = socket.local_addr()?;
        log::info!("udp tracker bound to {:?}", addr);
        let secret = random();
        Ok(Self {
            socket: Arc::new(socket),
            secret,
            tracker,
        })
    }
    pub async fn run(self) {
        loop {
            let mut packet = [0; MAX_PACKET_SIZE];
            match self.socket.recv_from(&mut packet).await {
                Ok((packet_len, addr)) => {
                    // ill-sized packets are ignored
                    if packet_len < MIN_PACKET_SIZE {
                        continue;
                    }
                    if packet_len > MAX_PACKET_SIZE {
                        log::trace!(
                            "packet too big: received packet of length {}, ignored",
                            packet_len,
                        );
                        continue;
                    }
                    log::trace!("received packet of length {}", packet_len);
                    let mut transaction = Transaction::new(
                        Arc::clone(&self.socket),
                        self.secret,
                        self.tracker.clone(),
                        packet,
                        packet_len,
                        addr,
                    );
                    // handle the request concurrently
                    tokio::spawn(async move {
                        if let Err(err) = transaction.handle().await {
                            log::error!("transaction handler failed: {}", err);
                        }
                    });
                }
                Err(err) => {
                    log::error!("unexpected io error while reading udp socket {}", err);
                }
            }
        }
    }
}
