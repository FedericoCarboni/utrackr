use std::io;

use tokio::net::ToSocketAddrs;

mod protocol;
mod transaction;
use protocol::UdpTracker;

pub struct Tracker {
    udp: UdpTracker,
}

impl Tracker {
    pub async fn bind<T: ToSocketAddrs>(addrs: T) -> io::Result<Self> {
        Ok(Self {
            udp: UdpTracker::bind(addrs).await?,
        })
    }
    pub async fn run(&self) -> io::Result<()> {
        self.udp.run_forever().await
    }
}
