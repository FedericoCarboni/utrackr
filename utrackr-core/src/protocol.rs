use std::{io, sync::Arc, time::Duration};

use tokio::{
    net::{ToSocketAddrs, UdpSocket},
    time,
};

use crate::transaction::{Transaction, MAX_PACKET_SIZE, MIN_PACKET_SIZE, SECRET_SIZE};

pub struct UdpTracker {
    socket: Arc<UdpSocket>,
    secret: [u8; SECRET_SIZE],
}

impl UdpTracker {
    pub async fn bind<T: ToSocketAddrs>(addrs: T) -> io::Result<Self> {
        let mut secret = [0u8; SECRET_SIZE];
        openssl::rand::rand_bytes(&mut secret)?;
        Ok(Self {
            socket: Arc::new(UdpSocket::bind(addrs).await?),
            secret,
        })
    }
    pub async fn run_forever(&self) -> io::Result<()> {
        log::info!("running");
        loop {
            let mut packet = [0u8; MAX_PACKET_SIZE];
            match self.socket.recv_from(&mut packet).await {
                Ok((packet_len, addr)) => {
                    // ill-sized packets are ignored
                    if packet_len < MIN_PACKET_SIZE {
                        continue;
                    }
                    log::trace!("received packet of length {}", packet_len);
                    let transaction =
                        Transaction::new(self.socket.clone(), self.secret, packet, packet_len, addr);
                    // handle the request concurrently
                    tokio::spawn(async move {
                        if let Err(err) = transaction.handle().await {
                            log::error!("unexpected error {}", err);
                        }
                    });
                }
                Err(err) => {
                    log::error!("{}", err);
                    log::info!("waiting 60 seconds before retrying");
                    time::sleep(Duration::from_secs(60)).await;
                }
            }
        }
    }
}
