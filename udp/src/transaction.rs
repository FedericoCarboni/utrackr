use std::{
    fmt, io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use arrayref::array_ref;
use blake3::Hasher;
use tokio::net::UdpSocket;

use utrackr_core::{Announce, Event, Tracker};

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

const MAX_NUM_WANT: usize = 512;

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

/// Turns IPv6 addressed mapped to IPv4 addresses to IPv4 addresses. Does nothing
/// to IPv4 addresses or proper IPv6 addresses. This is needed to support both
/// IPv4 and IPv6 at the same time.
fn to_canonical_ip(ip: IpAddr) -> IpAddr {
    match ip {
        ipv4 @ IpAddr::V4(_) => ipv4,
        IpAddr::V6(ipv6) => match ipv6.octets() {
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, a, b, c, d] => {
                IpAddr::V4(Ipv4Addr::new(a, b, c, d))
            }
            _ => IpAddr::V6(ipv6),
        },
    }
}

pub(crate) struct Transaction {
    socket: Arc<UdpSocket>,
    secret: [u8; SECRET_SIZE],
    tracker: Tracker,
    packet: [u8; MAX_PACKET_SIZE],
    packet_len: usize,
    addr: SocketAddr,
    instant: Instant,
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
            instant: Instant::now(),
        }
    }
    pub(crate) async fn handle(&mut self) -> io::Result<()> {
        if self.packet[8..12] == ACTION_CONNECT {
            if self.packet_len >= MIN_CONNECT_SIZE && &self.packet[0..8] == PROTOCOL_ID {
                // CONNECT packet
                log::trace!("CONNECT request from {}", self.addr);
                self.connect().await?;
            }
        } else if self.packet[8..12] == ACTION_ANNOUNCE {
            if self.packet_len >= MIN_ANNOUNCE_SIZE {
                log::trace!("ANNOUNCE request from {}", self.addr);
                if !self.verify_connection_id() {
                    log::trace!("ANNOUNCE request from {}, invalid connection_id", self.addr);
                    return self.error(b"Invalid connection id").await;
                }
                self.announce().await?;
            }
        } else if self.packet[8..12] == ACTION_SCRAPE {
            if self.packet_len >= MIN_SCRAPE_SIZE {
                log::trace!("SCRAPE request from {}", self.addr);
                if !self.verify_connection_id() {
                    log::trace!("SCRAPE request from {}, invalid connection_id", self.addr);
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
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        verify_connection_id(
            &self.secret,
            timestamp / 120,
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
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        make_connection_id(
            &self.secret,
            &(timestamp / 120).to_be_bytes(),
            &match self.addr.ip() {
                IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped(),
                IpAddr::V6(ipv6) => ipv6,
            }
            .octets(),
            &self.addr.port().to_be_bytes(),
        )
    }
    async fn error(&self, message: &[u8]) -> io::Result<()> {
        // make sure that we have a terminating 0 byte
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
        let info_hash = array_ref!(self.packet, 16, 20).clone();
        let peer_id = array_ref!(self.packet, 36, 20).clone();
        let downloaded = i64::from_be_bytes(array_ref!(self.packet, 56, 8).clone());
        let left = i64::from_be_bytes(array_ref!(self.packet, 64, 8).clone());
        let uploaded = i64::from_be_bytes(array_ref!(self.packet, 72, 8).clone());
        let event = i32::from_be_bytes(array_ref!(self.packet, 80, 4).clone());
        // the ip_address in the announce packet is currently ignored
        // let ip_address = &self.packet[84..88];
        let key = u32::from_be_bytes(array_ref!(self.packet, 88, 4).clone());
        let num_want = i32::from_be_bytes(array_ref!(self.packet, 92, 4).clone());
        let port = u16::from_be_bytes(array_ref!(self.packet, 96, 2).clone());

        let canonical_ip = to_canonical_ip(self.addr.ip());
        let announce = Announce {
            info_hash,
            peer_id,
            downloaded,
            uploaded,
            left,
            addr: SocketAddr::new(canonical_ip, port),
            key: Some(key),
            event: match event {
                1 => Event::Completed,
                2 => Event::Started,
                3 => Event::Stopped,
                _ => Event::None,
            },
            num_want,
            instant: self.instant,
        };

        if let Ok(opt) = self.tracker.announce(announce).await {
            let (seeders, leechers, addrs) = opt.unwrap_or((0, 0, vec![]));
            let mut rpkt = [0u8; ANNOUNCE_SIZE];
            // action ANNOUNCE
            rpkt[3] = 0x01;
            // transaction_id
            rpkt[4..8].copy_from_slice(&self.packet[12..16]);
            // interval
            rpkt[8..12].copy_from_slice(&1800i32.to_be_bytes());
            rpkt[12..16].copy_from_slice(&leechers.to_be_bytes());
            rpkt[16..20].copy_from_slice(&seeders.to_be_bytes());

            let mut offset = 20;

            for (_, addr) in addrs {
                if canonical_ip.is_ipv6() {
                    rpkt[offset..offset + 16].copy_from_slice(
                        &match addr.ip() {
                            IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped(),
                            IpAddr::V6(ipv6) => ipv6,
                        }
                        .octets(),
                    );
                    rpkt[offset + 16..offset + 18].copy_from_slice(&addr.port().to_be_bytes());
                    offset += 18;
                } else {
                    rpkt[offset..offset + 4].copy_from_slice(
                        &match addr.ip() {
                            IpAddr::V4(ipv4) => ipv4,
                            IpAddr::V6(ipv6) => ipv6.to_ipv4().unwrap(),
                        }
                        .octets(),
                    );
                    rpkt[offset + 4..offset + 6].copy_from_slice(&addr.port().to_be_bytes());
                    offset += 6;
                }
            }
            if let Err(error) = self.socket.send_to(&rpkt, self.addr).await {
                log::error!("failed to send ANNOUNCE response: {}", error);
            }
        } else {
            self.error(b"").await?;
        }
        Ok(())
    }
    async fn scrape(&mut self) -> io::Result<()> {
        let mut rpkt = [0u8; SCRAPE_SIZE];
        // action SCRAPE
        rpkt[3] = 0x02;
        // transaction_id
        rpkt[4..8].copy_from_slice(&self.packet[12..16]);
        let mut offset = 8;
        let mut i = 16;
        let mut info_hashes = Vec::with_capacity((self.packet_len - 8) / 20);
        while i < self.packet_len {
            let info_hash = array_ref!(self.packet, i, 20).clone();
            if info_hash == [0; 20] {
                break;
            }
            info_hashes.push(info_hash);
        }
        let torrents = self.tracker.scrape(&info_hashes).await;
        for (seeders, leechers, completed) in torrents {
            rpkt[offset..offset + 4].copy_from_slice(&seeders.to_be_bytes());
            rpkt[offset + 4..offset + 8].copy_from_slice(&completed.to_be_bytes());
            rpkt[offset + 8..offset + 12].copy_from_slice(&leechers.to_be_bytes());
            offset += 12;
            i += 20;
        }
        if let Err(error) = self.socket.send_to(&rpkt, self.addr).await {
            log::error!("failed to send SCRAPE response: {}", error);
        }
        Ok(())
    }
}
