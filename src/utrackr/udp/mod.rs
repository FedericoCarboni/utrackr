//! [UDP Tracker Protocol for BitTorrent](https://www.bittorrent.org/beps/bep_0015.html)
//! This module implements the UDP Tracker Protocol as specified by BEP 15.

use std::{
    io,
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
    time::Instant,
};

use rand::random;
use tokio::net::UdpSocket;

use crate::core::{Tracker, UdpConfig};
use crate::udp::protocol::{Secret, Transaction, MAX_PACKET_SIZE, MIN_PACKET_SIZE};

mod protocol;
mod path_and_query;

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
    /// Run the server indefinitely, this function is cancel safe.
    pub async fn run(self) {
        loop {
            let mut packet = [0; MAX_PACKET_SIZE];
            match self.socket.recv_from(&mut packet).await {
                Ok((packet_len, addr)) => {
                    // ill-sized packets are ignored
                    if packet_len < MIN_PACKET_SIZE {
                        log::trace!("packet too small: received packet of length {}", packet_len,);
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
                    let transaction = Transaction {
                        socket: Arc::clone(&self.socket),
                        secret: self.secret,
                        tracker: self.tracker.clone(),
                        remote_ip: match addr.ip() {
                            ipv4 @ IpAddr::V4(_) => ipv4,
                            ipv6 @ IpAddr::V6(v6) => match v6.octets() {
                                [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, a, b, c, d] => {
                                    IpAddr::V4(Ipv4Addr::new(a, b, c, d))
                                }
                                _ => ipv6,
                            },
                        },
                        packet,
                        packet_len,
                        addr,
                        instant: Instant::now(),
                    };
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
