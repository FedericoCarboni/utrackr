use std::{future::Future, io, net::{UdpSocket as StdUdpSocket, ToSocketAddrs, SocketAddr, IpAddr}, sync::Arc};

use rand::random;
use tokio::net::UdpSocket;

use transaction::{Transaction, MAX_PACKET_SIZE, MIN_PACKET_SIZE, SECRET_SIZE};
use utrackr_core::{Tracker, UdpConfig};

mod transaction;
mod sockopt;

pub struct UdpTracker {
    socket: Arc<UdpSocket>,
    secret: [u8; SECRET_SIZE],
    tracker: Tracker,
}

impl UdpTracker {
    pub async fn bind(tracker: Tracker, config: UdpConfig) -> io::Result<Self> {
        let socket = UdpSocket::bind(config.bind.addrs()).await?;
        let addr = socket.local_addr()?;
        if addr.is_ipv6() {
            sockopt::unset_ipv6_v6only(&socket)?;
        }
        log::info!("udp tracker bound to {:?}", addr);
        let secret: [u8; SECRET_SIZE] = random();
        Ok(Self {
            socket: Arc::new(socket),
            secret,
            tracker,
        })
    }
    pub async fn run_until(self, shutdown: impl Future) {
        tokio::select! {
            _ = shutdown => {
                log::info!("udp tracker gracefully shutting down");
            }
            _ = self.run() => {}
        }
    }
    pub async fn run(self) {
        loop {
            let mut packet = [0; MAX_PACKET_SIZE];
            fn to_canonical_ip(ip: IpAddr) -> IpAddr {
                match ip {
                    ipv4 @ IpAddr::V4(_) => ipv4,
                    IpAddr::V6(ipv6) => match ipv6.octets() {
                        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, a, b, c, d] => {
                            IpAddr::V4(std::net::Ipv4Addr::new(a, b, c, d))
                        }
                        _ => IpAddr::V6(ipv6),
                    },
                }
            }
            match self.socket.recv_from(&mut packet).await {
                Ok((packet_len, addr)) => {
                    log::trace!("{:?}", to_canonical_ip(addr.ip()).is_ipv6());
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
                    } else {
                        log::trace!("received packet of length {}", packet_len);
                    }
                    let mut transaction = Transaction::new(
                        Arc::clone(&self.socket),
                        self.secret.clone(),
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
