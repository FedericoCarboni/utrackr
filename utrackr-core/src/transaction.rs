use std::{
    fmt, io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use blake3::Hasher;
use tokio::net::UdpSocket;

// http://xbtt.sourceforge.net/udp_tracker_protocol.html
// https://www.bittorrent.org/beps/bep_0015.html
// https://www.libtorrent.org/udp_tracker_protocol.html

// XBT Tracker uses 2048, opentracker uses 8192, it could be tweaked for
// performance reasons
pub const MAX_PACKET_SIZE: usize = 8192;
// CONNECT is the smallest packet in the protocol
pub const MIN_PACKET_SIZE: usize = 16;

// This is used to prevent UDP spoofing, if anyone has to guess 8 random bytes
// then they might as well just try to guess the connection_id itself.
pub const SECRET_SIZE: usize = 8;

const DEFAULT_NUM_WANT: i32 = 64;
const MAX_NUM_WANT: i32 = 128;

const MIN_CONNECT_SIZE: usize = 16;
const MIN_ANNOUNCE_SIZE: usize = 98;
const MIN_SCRAPE_SIZE: usize = 36;

const CONNECT_SIZE: usize = 16;
const ANNOUNCE_SIZE: usize = 20 + 18 * MAX_NUM_WANT as usize;

const PROTOCOL_ID: &'static [u8] = &0x41727101980i64.to_be_bytes();

const ACTION_CONNECT: &'static [u8] = &0x0i32.to_be_bytes();
const ACTION_ANNOUNCE: &'static [u8] = &0x1i32.to_be_bytes();
const ACTION_SCRAPE: &'static [u8] = &0x2i32.to_be_bytes();
const ACTION_ERROR: &'static [u8] = &0x3i32.to_be_bytes();

/// The UDP tracker protocol specification recommends that connection ids have
/// two features:
///  - they should not be guessable by clients
///  - they should expire around every 2 minutes
/// `connection_id` is the first 8 bytes of the blake3 hash of `secret`,
/// `time_frame`, `ip` and `port`
fn make_connection_id(secret: &[u8; 8], time_frame: &[u8], ip: &[u8], port: &[u8]) -> [u8; 8] {
    let mut connection_id = [0u8; 8];
    let mut digest = Hasher::new();
    // the connection_id must not be guessable by clients
    digest.update(secret);
    // the connection_id should be invalidated every 2 minutes
    digest.update(time_frame);
    // the connection_id should change based on ip and port of the client
    digest.update(ip);
    digest.update(port);
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

pub struct Transaction {
    socket: Arc<UdpSocket>,
    secret: [u8; SECRET_SIZE],
    packet: [u8; MAX_PACKET_SIZE],
    packet_len: usize,
    addr: SocketAddr,
    timestamp: u64,
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
    pub fn new(
        socket: Arc<UdpSocket>,
        secret: [u8; SECRET_SIZE],
        packet: [u8; MAX_PACKET_SIZE],
        packet_len: usize,
        addr: SocketAddr,
    ) -> Self {
        debug_assert!(packet_len >= MIN_PACKET_SIZE);
        Self {
            socket,
            secret,
            packet,
            packet_len,
            addr,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_secs(),
        }
    }
    pub async fn handle(&self) -> io::Result<()> {
        match &self.packet[8..12] {
            ACTION_CONNECT
                if self.packet_len >= MIN_CONNECT_SIZE && &self.packet[0..8] == PROTOCOL_ID =>
            {
                // CONNECT packet
                log::debug!("CONNECT request from {}", self.addr);
                self.connect().await?;
            }
            ACTION_ANNOUNCE if self.packet_len >= MIN_ANNOUNCE_SIZE => {
                if !self.verify_connection_id() {
                    return self.access_denied().await;
                }
                // ANNOUNCE packet
                log::debug!("ANNOUNCE request from {}", self.addr);
                self.announce().await?;
            }
            ACTION_SCRAPE if self.packet_len >= MIN_SCRAPE_SIZE => {
                if !self.verify_connection_id() {
                    return self.access_denied().await;
                }
                // SCRAPE packet
                log::debug!("SCRAPE request from {}", self.addr);
                self.scrape().await?;
            }
            _ => {
                // invalid or unknown packet
                log::trace!(
                    "unknown packet ({} bytes) from {}",
                    self.packet_len,
                    self.addr
                );
            }
        }
        Ok(())
    }
    fn verify_connection_id(&self) -> bool {
        verify_connection_id(
            &self.secret,
            self.timestamp / 120,
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
            &(self.timestamp / 120).to_be_bytes(),
            &match self.addr.ip() {
                IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped(),
                IpAddr::V6(ipv6) => ipv6,
            }
            .octets(),
            &self.addr.port().to_be_bytes(),
        )
    }
    async fn access_denied(&self) -> io::Result<()> {
        log::info!("access denied for {}", self.addr);

        let mut rpkt = [0u8; 22];
        rpkt[0..4].copy_from_slice(ACTION_ERROR);
        rpkt[4..8].copy_from_slice(&self.packet[12..16]);
        rpkt[8..22].copy_from_slice(b"access denied\0");

        self.socket.send_to(&rpkt, self.addr).await?;
        Ok(())
    }
    async fn connect(&self) -> io::Result<()> {
        debug_assert!(self.packet_len >= MIN_CONNECT_SIZE);
        debug_assert!(&self.packet[0..8] == PROTOCOL_ID);

        let mut rpkt = [0u8; CONNECT_SIZE];
        // action
        rpkt[0..4].copy_from_slice(ACTION_CONNECT);
        // transaction_id
        rpkt[4..8].copy_from_slice(&self.packet[12..16]);
        // connection_id
        rpkt[8..16].copy_from_slice(&self.connection_id());

        self.socket.send_to(&rpkt, self.addr).await?;
        Ok(())
    }
    async fn announce(&self) -> io::Result<()> {
        debug_assert!(self.packet_len >= MIN_ANNOUNCE_SIZE);
        
        let info_hash = &self.packet[16..36];
        let peer_id = &self.packet[36..56];
        let downloaded = u64::from_be_bytes(self.packet[56..64].try_into().unwrap());
        let left = u64::from_be_bytes(self.packet[64..72].try_into().unwrap());
        let uploaded = u64::from_be_bytes(self.packet[72..80].try_into().unwrap());
        let event = i32::from_be_bytes(self.packet[80..84].try_into().unwrap());
        // ip_address is ignored
        // let ip_address = 0;
        let key = &self.packet[88..92];
        let mut num_want = i32::from_be_bytes(self.packet[92..96].try_into().unwrap());
        if num_want < 0 {
            num_want = DEFAULT_NUM_WANT;
        } else if num_want > MAX_NUM_WANT {
            num_want = MAX_NUM_WANT;
        }
        let num_want = num_want as usize;
        let port = u16::from_be_bytes(self.packet[96..98].try_into().unwrap());
        
        let mut rpkt = [0u8; ANNOUNCE_SIZE];
        // action
        rpkt[0..4].copy_from_slice(ACTION_ANNOUNCE);
        // transaction_id
        rpkt[4..8].copy_from_slice(&self.packet[12..16]);
        // interval
        rpkt[8..12].copy_from_slice(&[0u8, 4]);
        // 
        rpkt[12..16].copy_from_slice(&[0u8; 4]);
        rpkt[16..20].copy_from_slice(&[0u8; 4]);
        
        self.socket.send_to(&rpkt, self.addr).await?;
        Ok(())
    }
    async fn scrape(&self) -> io::Result<()> {


        Ok(())
    }
}
