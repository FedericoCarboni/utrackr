use std::{io, sync::Arc, time::Duration};

use tokio::{
    net::{ToSocketAddrs, UdpSocket},
    time,
};

use crate::transaction::{Transaction, MAX_PACKET_SIZE, MIN_PACKET_SIZE, SECRET_SIZE};

pub struct UdpTracker {
    socket: UdpSocket,
    secret: [u8; SECRET_SIZE],
}

impl UdpTracker {
    pub async fn bind<T: ToSocketAddrs>(addrs: T) -> io::Result<Self> {
        let mut secret = [0u8; SECRET_SIZE];
        openssl::rand::rand_bytes(&mut secret)?;
        Ok(Self {
            socket: UdpSocket::bind(addrs).await?,
            secret,
        })
    }
    pub async fn run(self) -> io::Result<()> {
        log::info!("running");
        let socket = Arc::new(self.socket);
        loop {
            let mut packet = [0u8; MAX_PACKET_SIZE];
            match socket.recv_from(&mut packet).await {
                Ok((mut packet_len, addr)) => {
                    // ill-sized packets are ignored
                    if packet_len < MIN_PACKET_SIZE {
                        continue;
                    }
                    if packet_len > MAX_PACKET_SIZE {
                        log::trace!(
                            "packet too big: received packet of length {}, it has been truncated to {}",
                            packet_len,
                            MAX_PACKET_SIZE
                        );
                        packet_len = MAX_PACKET_SIZE;
                    } else {
                        log::trace!("received packet of length {}", packet_len);
                    }
                    let transaction = Transaction::new(
                        Arc::clone(&socket),
                        self.secret,
                        packet,
                        packet_len,
                        addr,
                    );
                    // handle the request concurrently
                    tokio::spawn(async move {
                        if let Err(err) = transaction.handle().await {
                            log::error!("unexpected error {}", err);
                        }
                    });
                }
                Err(err) => {
                    log::error!("unexpected io error while reading udp socket {}", err);
                    log::info!("waiting 60 seconds before retrying");
                    time::sleep(Duration::from_secs(60)).await;
                }
            }
        }
    }
}
