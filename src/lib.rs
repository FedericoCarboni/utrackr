use tokio::net::UdpSocket;

use std::net::{SocketAddr, IpAddr};
use std::io;

const MAX_PKT_SIZE: usize = 2048;
const ERR_PKT_SIZE: usize = 64;
const MIN_PKT_SIZE: usize = 16;
const MIN_CONNECT_SIZE: usize = 16;
const MIN_ANNOUNCE_SIZE: usize = 98;
const MIN_SCRAPE_SIZE: usize = 98;

const MAGIC_CONNECTION_ID: &'static [u8] = &0x41727101980i64.to_be_bytes();

const ACTION_CONNECT: &'static [u8] = &0x0i32.to_be_bytes();
const ACTION_ANNOUNCE: &'static [u8] = &0x1i32.to_be_bytes();
const ACTION_SCRAPE: &'static [u8] = &0x2i32.to_be_bytes();
const ACTION_ERROR: &'static [u8] = &0x3i32.to_be_bytes();

#[derive(Debug)]
struct AnnouncePacket<'a>(&'a [u8]);

#[inline(always)]
fn i64_at_offset(b: &[u8], offset: usize) -> i64 {
    i64::from_be_bytes([
        b[offset + 0], b[offset + 1], b[offset + 2], b[offset + 3],
        b[offset + 4], b[offset + 5], b[offset + 6], b[offset + 7],
    ])
}

impl<'a> AnnouncePacket<'a> {
    #[inline]
    pub fn connection_id(&self) -> &[u8] {
        &self.0[0..8]
    }
    #[inline]
    pub fn transaction_id(&self) -> i32 {
        i32::from_be_bytes([
            self.0[12], self.0[13], self.0[14], self.0[15],
        ])
    }
    #[inline]
    pub fn info_hash(&self) -> &[u8] {
        &self.0[16..36]
    }
    #[inline]
    pub fn peer_id(&self) -> &[u8] {
        &self.0[36..56]
    }
    #[inline]
    pub fn downloaded(&self) -> i64 {
        i64_at_offset(self.0, 56)
    }
    #[inline]
    pub fn left(&self) -> i64 {
        i64_at_offset(self.0, 64)
    }
    #[inline]
    pub fn uploaded(&self) -> i64 {
        i64_at_offset(self.0, 72)
    }
    #[inline]
    pub fn event(&self) -> &[u8] {
        &self.0[80..84]
    }
    #[inline]
    pub fn key(&self) -> &[u8] {
        &self.0[88..92]
    }
    #[inline]
    pub fn num_want(&self) -> i32 {
        i32::from_be_bytes([
            self.0[92], self.0[93], self.0[94], self.0[95],
        ])
    }
}

#[derive(Debug)]
pub struct UdpTrackerServer {
    socket: UdpSocket,
    secret: [u8; 16],
}

impl UdpTrackerServer {
    pub async fn run(&self) -> io::Result<()> {
        loop {
            let mut pkt = [0u8; MAX_PKT_SIZE];
            let (n, addr) = self.socket.recv_from(&mut pkt).await?;
            dbg!(n);

            if n < MIN_PKT_SIZE {
                continue;
            }

            let pkt = &pkt[..n];
            dbg!(pkt);
            dbg!(&pkt[8..12]);
            dbg!(ACTION_CONNECT);
            dbg!(&pkt[8..12] == ACTION_CONNECT);

            match &pkt[8..12] {
                ACTION_CONNECT if n >= MIN_CONNECT_SIZE => self.connect(pkt, addr).await?,
                ACTION_ANNOUNCE if n >= MIN_ANNOUNCE_SIZE => self.announce(pkt, addr).await?,
                ACTION_SCRAPE if n >= MIN_SCRAPE_SIZE => self.scrape(pkt, addr).await?,
                _ => {},
            }
        }
    }
    fn connection_id(&self, addr: SocketAddr) -> [u8; 8] {
        let mut c = crc64fast::Digest::new();
        c.write(&self.secret);
        c.write(&match addr.ip() {
            IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped(),
            IpAddr::V6(ipv6) => ipv6,
        }.octets());
        c.write(&addr.port().to_ne_bytes());
        c.sum64().to_ne_bytes()
    }
    async fn connect(&self, pkt: &[u8], addr: SocketAddr) -> io::Result<()> {
        if &pkt[..8] != MAGIC_CONNECTION_ID {
            return Ok(());
        }
        println!("connect");
        let conn_id = self.connection_id(addr);
        dbg!(conn_id);

        let mut rpkt = [0u8; 16];
        rpkt[0..4].copy_from_slice(ACTION_CONNECT);
        rpkt[4..8].copy_from_slice(&pkt[12..16]);
        rpkt[8..16].copy_from_slice(&conn_id);

        self.socket.send_to(&rpkt, addr).await?;
        Ok(())
    }
    async fn announce(&self, pkt: &[u8], addr: SocketAddr) -> io::Result<()> {
        let packet = AnnouncePacket(pkt);
        println!("announce");

        dbg!(packet.connection_id());
        if packet.connection_id() != self.connection_id(addr) {
            return self.deny_access(pkt, addr).await;
        }

        Ok(())
    }
    async fn scrape(&self, pkt: &[u8], addr: SocketAddr) -> io::Result<()> {

        Ok(())
    }
    async fn deny_access(&self, pkt: &[u8], addr: SocketAddr) -> io::Result<()> {
        let mut rpkt = [0u8; 22];
        rpkt[0..4].copy_from_slice(ACTION_ERROR);
        rpkt[4..8].copy_from_slice(&pkt[12..16]);
        rpkt[8..22].copy_from_slice(b"access denied\0");

        self.socket.send_to(&rpkt, addr).await?;
        Ok(())
    }
    async fn send_error(&self, msg: &[u8], pkt: &[u8], addr: SocketAddr)  -> io::Result<()> {
        let mut rpkt = [0u8; ERR_PKT_SIZE];
        rpkt[0..4].copy_from_slice(ACTION_ERROR);
        rpkt[4..8].copy_from_slice(&pkt[12..16]);
        rpkt[8..msg.len()].copy_from_slice(msg);

        self.socket.send_to(&rpkt, addr).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server() {
        let mut secret = [0u8; 16];
        openssl::rand::rand_bytes(&mut secret).unwrap();
        let server = UdpTrackerServer {
            socket: UdpSocket::bind("127.0.0.1:9000").await.unwrap(),
            secret,
        };
        server.run().await.unwrap();
    }
}
