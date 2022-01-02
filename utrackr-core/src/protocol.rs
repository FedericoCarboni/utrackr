use std::{io, sync::Arc};

use rand::random;
use tokio::{
    net::{ToSocketAddrs, UdpSocket},
    sync::RwLock,
};

use crate::tracker::Tracker;
use crate::transaction::{Transaction, MAX_PACKET_SIZE, MIN_PACKET_SIZE, SECRET_SIZE};

#[derive(Debug)]
pub struct UdpTracker {
    socket: Arc<UdpSocket>,
    secret: [u8; SECRET_SIZE],
    tracker: Arc<RwLock<Tracker>>,
}

impl UdpTracker {
    pub async fn bind<T: ToSocketAddrs>(addrs: T) -> io::Result<Self> {
        let secret: [u8; SECRET_SIZE] = random();
        Ok(Self {
            socket: Arc::new(UdpSocket::bind(addrs).await?),
            secret,
            tracker: Arc::new(RwLock::new(Tracker::new())),
        })
    }
    pub async fn run(self) -> io::Result<()> {
        loop {
            let mut packet = [0u8; MAX_PACKET_SIZE];
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
