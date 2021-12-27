use std::{
    fmt, io,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use blake3::Hasher;
use tokio::net::UdpSocket;

use crate::tracker::{Peer, Tracker};

// http://xbtt.sourceforge.net/udp_tracker_protocol.html
// https://www.bittorrent.org/beps/bep_0015.html
// https://www.libtorrent.org/udp_tracker_protocol.html

// XBT Tracker uses 2048, opentracker uses 8192, it could be tweaked for
// performance reasons
pub(crate) const MAX_PACKET_SIZE: usize = 8192;
// CONNECT is the smallest packet in the protocol
pub(crate) const MIN_PACKET_SIZE: usize = 16;

// This is used to prevent UDP spoofing, if anyone has to guess 8 random bytes
// then they might as well just try to guess the connection_id itself.
pub(crate) const SECRET_SIZE: usize = 8;

const DEFAULT_NUM_WANT: i32 = 64;
const MAX_NUM_WANT: i32 = 128;

const MIN_CONNECT_SIZE: usize = 16;
const MIN_ANNOUNCE_SIZE: usize = 98;
const MIN_SCRAPE_SIZE: usize = 36;

const CONNECT_SIZE: usize = 16;
const ANNOUNCE_SIZE: usize = 20 + 18 * MAX_NUM_WANT as usize;
const SCRAPE_SIZE: usize = 8 + 12 * MAX_NUM_WANT as usize;

const PROTOCOL_ID: [u8; 8] = 0x41727101980i64.to_be_bytes();

const ACTION_CONNECT: [u8; 4] = 0x0i32.to_be_bytes();
const ACTION_ANNOUNCE: [u8; 4] = 0x1i32.to_be_bytes();
const ACTION_SCRAPE: [u8; 4] = 0x2i32.to_be_bytes();
// const ACTION_ERROR: [u8; 4] = 0x3i32.to_be_bytes();

macro_rules! array_from_slice {
    ($slice:expr, $start:expr => $end:expr) => {{
        let mut r = [0u8; $end - $start];
        for i in 0..$end - $start {
            r[i] = $slice[$start..][i];
        }
        r
    }};
}

/// The UDP tracker protocol specification recommends that connection ids have
/// two features:
///  - they should not be guessable by clients
///  - they should expire around every 2 minutes
/// `connection_id` is the first 8 bytes of the blake3 hash of `secret`,
/// `time_frame`, `ip` and `port`
fn make_connection_id(secret: &[u8; 8], time_frame: &[u8], ip: &[u8], _port: &[u8]) -> [u8; 8] {
    let mut connection_id = [0u8; 8];
    let mut digest = Hasher::new();
    // the connection_id must not be guessable by clients
    digest.update(secret);
    // the connection_id should be invalidated every 2 minutes
    digest.update(time_frame);
    // the connection_id should change based on ip of the client
    digest.update(ip);
    // digest.update(port);
    let hash = digest.finalize();
    connection_id.copy_from_slice(&hash.as_bytes()[0..8]);
    connection_id
}

fn verify_connection_id(
    secret: &[u8; 8],
    time_frame: u64,
    ip: &[u8],
    port: &[u8],
    connection_id: &[u8],
) -> bool {
    connection_id == make_connection_id(secret, &time_frame.to_be_bytes(), ip, port)
        || connection_id == make_connection_id(secret, &(time_frame - 1).to_be_bytes(), ip, port)
}

pub(crate) struct Transaction {
    socket: Arc<UdpSocket>,
    secret: [u8; SECRET_SIZE],
    tracker: Tracker,
    packet: [u8; MAX_PACKET_SIZE],
    packet_len: usize,
    addr: SocketAddr,
    timestamp: std::time::Duration,
}

impl fmt::Debug for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transaction")
            .field("socket", &self.socket)
            .field("secret", &"<hidden>")
            .field("packet", &&self.packet[..self.packet_len])
            .field("addr", &self.addr)
            .finish()
    }
}

impl Transaction {
    pub(crate) fn new(
        socket: Arc<UdpSocket>,
        secret: [u8; SECRET_SIZE],
        tracker: Tracker,
        packet: [u8; MAX_PACKET_SIZE],
        packet_len: usize,
        addr: SocketAddr,
    ) -> Self {
        debug_assert!(packet_len >= MIN_PACKET_SIZE);
        Self {
            socket,
            secret,
            tracker,
            packet,
            packet_len,
            addr,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards"),
        }
    }
    pub(crate) async fn handle(&mut self) -> io::Result<()> {
        if self.packet[8..12] == ACTION_CONNECT {
            if self.packet_len >= MIN_CONNECT_SIZE && &self.packet[0..8] == PROTOCOL_ID {
                // CONNECT packet
                log::debug!("CONNECT request from {}", self.addr);
                self.connect().await?;
            }
        } else if self.packet[8..12] == ACTION_ANNOUNCE {
            if self.packet_len >= MIN_ANNOUNCE_SIZE {
                log::debug!("ANNOUNCE request from {}", self.addr);
                if !self.verify_connection_id() {
                    log::debug!("ANNOUNCE request from {}, invalid connection_id", self.addr);
                    return self.error(b"Invalid connection id").await;
                }
                self.announce().await?;
            }
        } else if self.packet[8..12] == ACTION_SCRAPE {
            if self.packet_len >= MIN_SCRAPE_SIZE {
                log::debug!("SCRAPE request from {}", self.addr);
                if !self.verify_connection_id() {
                    log::debug!("SCRAPE request from {}, invalid connection_id", self.addr);
                    return self.error(b"Invalid connection id").await;
                }
                self.scrape().await?;
            }
        } else {
            log::trace!("unknown packet ({} bytes)", self.packet_len);
        }
        Ok(())
    }
    fn verify_connection_id(&self) -> bool {
        verify_connection_id(
            &self.secret,
            self.timestamp.as_secs() / 120,
            &match self.addr.ip() {
                IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped(),
                IpAddr::V6(ipv6) => ipv6,
            }
            .octets(),
            &self.addr.port().to_be_bytes(),
            &self.packet[0..8],
        )
    }
    fn connection_id(&self) -> [u8; 8] {
        make_connection_id(
            &self.secret,
            &(self.timestamp.as_secs() / 120).to_be_bytes(),
            &match self.addr.ip() {
                IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped(),
                IpAddr::V6(ipv6) => ipv6,
            }
            .octets(),
            &self.addr.port().to_be_bytes(),
        )
    }
    async fn error(&self, message: &[u8]) -> io::Result<()> {
        debug_assert!(message.len() <= 55, "error message too long");

        let mut rpkt = [0u8; 64];
        // action ERROR
        rpkt[3] = 0x03;
        // transaction_id
        rpkt[4..8].copy_from_slice(&self.packet[12..16]);
        // C0-terminated human readable error message
        rpkt[8..8 + message.len()].copy_from_slice(message);

        if let Err(error) = self.socket.send_to(&rpkt, self.addr).await {
            log::error!("failed to send CONNECT response: {}", error);
        }
        Ok(())
    }
    async fn connect(&self) -> io::Result<()> {
        debug_assert!(self.packet_len >= MIN_CONNECT_SIZE);
        debug_assert!(&self.packet[0..8] == PROTOCOL_ID);

        // action CONNECT is all 0
        let mut rpkt = [0u8; CONNECT_SIZE];
        // transaction_id
        rpkt[4..8].copy_from_slice(&self.packet[12..16]);
        // connection_id
        rpkt[8..16].copy_from_slice(&self.connection_id());

        if let Err(error) = self.socket.send_to(&rpkt, self.addr).await {
            log::error!("failed to send CONNECT response: {}", error);
        }
        Ok(())
    }
    async fn announce(&mut self) -> io::Result<()> {
        debug_assert!(self.packet_len >= MIN_ANNOUNCE_SIZE);

        log::trace!(
            "announcing in {} mode",
            match self.addr.ip() {
                IpAddr::V4(_) => "ipv4",
                IpAddr::V6(_) => "ipv6",
            }
        );

        let info_hash = array_from_slice!(self.packet, 16 => 36);
        let peer_id = array_from_slice!(self.packet, 36 => 56);
        let downloaded = u64::from_be_bytes(array_from_slice!(self.packet, 56 => 64));
        let left = u64::from_be_bytes(array_from_slice!(self.packet, 64 => 72));
        let uploaded = u64::from_be_bytes(array_from_slice!(self.packet, 72 => 80));
        let event = i32::from_be_bytes(array_from_slice!(self.packet, 80 => 84));
        // the ip_address in the announce packet is currently ignored
        // let ip_address = &self.packet[84..88];
        // let key = &self.packet[88..92];
        let num_want = i32::from_be_bytes(array_from_slice!(self.packet, 92 => 96));
        let num_want = if num_want < 0 {
            DEFAULT_NUM_WANT
        } else if num_want > MAX_NUM_WANT {
            MAX_NUM_WANT
        } else {
            num_want
        };
        let port = u16::from_be_bytes(array_from_slice!(self.packet, 96 => 98));

        let peer = Peer {
            peer_id,
            info_hash,
            last_seen: self.timestamp,
            downloaded,
            uploaded,
            left,
            event,
            ip: match self.addr.ip() {
                IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped().octets(),
                IpAddr::V6(ipv6) => ipv6.octets(),
            },
            port,
            is_ipv4: self.addr.is_ipv4(),
        };

        let mut rpkt = [0u8; ANNOUNCE_SIZE];
        // action ANNOUNCE
        rpkt[3] = 0x01;
        // transaction_id
        rpkt[4..8].copy_from_slice(&self.packet[12..16]);
        self.tracker.announce(&peer, num_want, &mut rpkt).await.unwrap();

        if let Err(error) = self.socket.send_to(&rpkt, self.addr).await {
            log::error!("failed to send ANNOUNCE response: {}", error);
        }

        self.tracker.add_torrent_or_peer(peer).await;
        dbg!(&self.tracker.torrents.read().await);
        dbg!(&self.tracker.peers_.read().await);

        Ok(())
    }
    async fn scrape(&mut self) -> io::Result<()> {
        let mut rpkt = [0u8; SCRAPE_SIZE];
        // action SCRAPE
        rpkt[3] = 0x02;
        // transaction_id
        rpkt[4..8].copy_from_slice(&self.packet[12..16]);
        let offset = 8;
        let i = 16;
        let info_hash = &self.packet[i..i + 20];
        let (seeders, leechers, completed) = self
            .tracker
            .scrape(info_hash.try_into().unwrap())
            .await
            .unwrap();
        rpkt[offset..offset + 4].copy_from_slice(&seeders.to_be_bytes());
        rpkt[offset + 4..offset + 8].copy_from_slice(&completed.to_be_bytes());
        rpkt[offset + 8..offset + 12].copy_from_slice(&leechers.to_be_bytes());
        if let Err(error) = self.socket.send_to(&rpkt, self.addr).await {
            log::error!("failed to send SCRAPE response: {}", error);
        }
        Ok(())
    }
}
