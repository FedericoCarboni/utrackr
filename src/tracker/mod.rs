use std::net::SocketAddr;
use std::future::Future;
use std::io;

use log::{info, warn};

pub struct AnnounceReply {
    inner: [u8; 2048],
    offset: usize,
}

impl AnnounceReply {
    pub(crate) fn new(transaction_id: &[u8]) -> Self {
        let mut inner = [0u8; 2048];
        inner[0..4].copy_from_slice(crate::ACTION_ANNOUNCE);
        inner[4..8].copy_from_slice(transaction_id);
        Self {
            inner,
            offset: 20
        }
    }
    #[inline]
    pub fn set_interval(&mut self, interval: i32) -> &mut Self {
        self.inner[8..12].copy_from_slice(&interval.to_be_bytes());
        self
    }
    #[inline]
    pub fn set_leechers(&mut self, leechers: i32) -> &mut Self {
        self.inner[12..16].copy_from_slice(&leechers.to_be_bytes());
        self
    }
    #[inline]
    pub fn set_seeders(&mut self, seeders: i32) -> &mut Self {
        self.inner[16..20].copy_from_slice(&seeders.to_be_bytes());
        self
    }
    #[inline]
    pub fn push_peer(&mut self, ip: i32, port: u16) -> &mut Self {
        self.inner[self.offset..(self.offset + 4)].copy_from_slice(&ip.to_be_bytes());
        self.inner[(self.offset + 4)..(self.offset + 6)].copy_from_slice(&port.to_be_bytes());
        self.offset += 6;
        self
    }
    #[inline]
    pub fn add_peers<T: AsRef<(i32, u16)>>(&mut self, peers: &[T]) -> &mut Self {
        for peer in peers {
            let (ip, port) = peer.as_ref();
            self.push_peer(*ip, *port);
        }
        self
    }
}

pub trait Tracker {
    fn connection_id(&self, addr: SocketAddr) -> [u8; 8];
    fn announce(&self, reply: &mut AnnounceReply) -> dyn Future<Output = io::Result<()>>;
    fn scrape(&self, ) -> dyn Future<Output = io::Result<()>>;
}
