use std::{
    fmt, io,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use arrayref::array_ref;
use ring::digest;
use tokio::net::UdpSocket;

use crate::core::extensions::TrackerExtension;
use crate::core::{
    AnnounceParams, EmptyParamsParser, Error, Event, ParamsParser, Tracker, MAX_NUM_WANT,
};

use crate::udp::extensions::parse_extensions;

/// XBT Tracker uses 2048, opentracker uses 8192, it could be tweaked for
/// performance reasons
pub(in crate::udp) const MAX_PACKET_SIZE: usize = 2048;
/// CONNECT is the smallest packet in the protocol
pub(in crate::udp) const MIN_PACKET_SIZE: usize = MIN_CONNECT_SIZE;

/// The secret is used generate `connection_id`, to prevent UDP sender address
/// spoofing. 8 bytes should be enough, if an attacker has to guess 8 bytes they
/// might as well try to guess the `connection_id` itself.
pub(in crate::udp) type Secret = [u8; 8];

/// This is a hard-coded maximum value for the number of torrents that can be
/// scraped with a single UDP packet.
/// BEP 15 states `Up to about 74 torrents can be scraped at once. A full scrape
/// can't be done with this protocol.`
/// If clients need to scrape more torrents they can just send more than one
/// SCRAPE packet.
pub(in crate::udp) const MAX_SCRAPE_TORRENTS: usize = 80;

pub const MIN_CONNECT_SIZE: usize = 16;
pub const MIN_ANNOUNCE_SIZE: usize = 98;
pub const MIN_SCRAPE_SIZE: usize = 36;

pub const CONNECT_SIZE: usize = 16;
pub const ANNOUNCE_SIZE: usize = 20 + 18 * MAX_NUM_WANT;
pub const SCRAPE_SIZE: usize = 8 + 12 * MAX_SCRAPE_TORRENTS;

pub const PROTOCOL_ID: [u8; 8] = 0x41727101980i64.to_be_bytes();

pub const ACTION_CONNECT: [u8; 4] = 0x0i32.to_be_bytes();
pub const ACTION_ANNOUNCE: [u8; 4] = 0x1i32.to_be_bytes();
pub const ACTION_SCRAPE: [u8; 4] = 0x2i32.to_be_bytes();
// const ACTION_ERROR: [u8; 4] = 0x3i32.to_be_bytes();

#[inline]
fn ip_to_bytes(ip: &IpAddr) -> [u8; 16] {
    match ip {
        IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped().octets(),
        IpAddr::V6(ipv6) => ipv6.octets(),
    }
}

/// The UDP Tracker Protocol specification recommends that the connection id has
/// two properties:
///  - it should not be guessable by clients
///  - it should be accepted for at least 2 minutes after it's generated
/// The `connection_id` generated is the first 8 bytes of the SHA-2 hash of the
/// concatenation of `secret`, `two_min_window` and `remote_ip`.
#[inline]
fn make_connection_id(secret: &Secret, two_min_window: u64, remote_ip: &[u8; 16]) -> [u8; 8] {
    let mut data = [0u8; 32];
    data[0..8].copy_from_slice(secret);
    data[8..16].copy_from_slice(&two_min_window.to_be_bytes());
    data[16..32].copy_from_slice(remote_ip);
    let sha2 = digest::digest(&digest::SHA256, &data);
    // connection_id is only 8 bytes
    *array_ref!(sha2.as_ref(), 0, 8)
}

/// Verifies a connection id, returns true if it is valid, false otherwise.
#[inline]
fn verify_connection_id(
    secret: &[u8; 8],
    time_frame: u64,
    remote_ip: &IpAddr,
    connection_id: &[u8; 8],
) -> bool {
    let ip_bytes = ip_to_bytes(remote_ip);
    *connection_id == make_connection_id(secret, time_frame, &ip_bytes)
        || *connection_id == make_connection_id(secret, time_frame - 1, &ip_bytes)
}

#[inline]
fn two_min_window() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("have we traveled back in time?")
        .as_secs()
        / 120
}

pub struct Transaction<Extension, Config = (), Params = (), P = EmptyParamsParser>
where
    Extension: TrackerExtension<Config, Params, P> + Sync + Send,
    Config: Default + Sync + Send,
    Params: Sync + Send,
    P: ParamsParser<Params> + Sync + Send,
{
    pub(in crate::udp) socket: Arc<UdpSocket>,
    pub(in crate::udp) tracker: Arc<Tracker<Extension, Config, Params, P>>,
    pub(in crate::udp) secret: Secret,
    pub(in crate::udp) packet: [u8; MAX_PACKET_SIZE],
    pub(in crate::udp) packet_len: usize,
    pub(in crate::udp) remote_ip: IpAddr,
    pub(in crate::udp) addr: SocketAddr,
}

impl<Extension, Config, Params, P> fmt::Debug for Transaction<Extension, Config, Params, P>
where
    Extension: TrackerExtension<Config, Params, P> + Sync + Send,
    Config: Default + Sync + Send,
    Params: Sync + Send,
    P: ParamsParser<Params> + Sync + Send,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transaction")
            .field("socket", &self.socket)
            .field("secret", &"[secret]")
            .field("packet", &&self.packet[..self.packet_len])
            .field("addr", &self.addr)
            .finish()
    }
}

impl<Extension, Config, Params, P> Transaction<Extension, Config, Params, P>
where
    Extension: TrackerExtension<Config, Params, P> + Sync + Send,
    Config: Default + Sync + Send,
    Params: Sync + Send,
    P: ParamsParser<Params> + Sync + Send,
{
    #[inline]
    fn connection_id(&self) -> [u8; 8] {
        make_connection_id(
            &self.secret,
            two_min_window(),
            &ip_to_bytes(&self.remote_ip),
        )
    }
    #[inline]
    fn verify_connection_id(&self) -> bool {
        verify_connection_id(
            &self.secret,
            two_min_window(),
            &self.remote_ip,
            array_ref!(self.packet, 0, 8),
        )
    }
    pub(in crate::udp) async fn handle(&self) -> io::Result<()> {
        if self.packet[8..12] == ACTION_CONNECT {
            if self.packet_len >= MIN_CONNECT_SIZE && self.packet[0..8] == PROTOCOL_ID {
                // CONNECT packet
                log::trace!("CONNECT request from {}", self.addr);
                self.connect().await?;
            }
        } else if self.packet[8..12] == ACTION_ANNOUNCE {
            if self.packet_len >= MIN_ANNOUNCE_SIZE {
                log::trace!("ANNOUNCE request from {}", self.addr);
                if !self.verify_connection_id() {
                    log::trace!("ANNOUNCE request from {}, invalid connection_id", self.addr);
                    return self.error(Error::AccessDenied.message()).await;
                }
                if let Err(err) = self.announce().await {
                    return self.error(err.message()).await;
                }
            }
        } else if self.packet[8..12] == ACTION_SCRAPE {
            if self.packet_len >= MIN_SCRAPE_SIZE {
                log::trace!("SCRAPE request from {}", self.addr);
                if !self.verify_connection_id() {
                    log::trace!("SCRAPE request from {}, invalid connection_id", self.addr);
                    return self.error(Error::AccessDenied.message()).await;
                }
                self.scrape().await?;
            }
        } else {
            log::trace!("unknown packet ({} bytes)", self.packet_len);
        }
        Ok(())
    }
    /// Sends an error packet to the requesting client.
    /// We don't make any assumptions about clients, so all error messages
    /// should be printable ASCII characters.
    async fn error(&self, message: &str) -> io::Result<()> {
        // make sure that we have a terminating 0 byte
        debug_assert!(message.len() <= 55, "error message too long");
        dbg!(message);
        // make sure that the error message contains only printable ascii chars
        debug_assert!(
            message.bytes().all(|b| (0x20..=0x7E).contains(&b)),
            "error message contains non-ascii or non-printable ascii"
        );

        let mut rpkt = [0u8; 64];
        // action ERROR
        rpkt[3] = 0x03;
        // transaction_id
        rpkt[4..8].copy_from_slice(&self.packet[12..16]);
        // C0-terminated human readable error message
        rpkt[8..8 + message.len()].copy_from_slice(message.as_bytes());

        if let Err(error) = self
            .socket
            .send_to(&rpkt[..message.len() + 9], self.addr)
            .await
        {
            log::error!("failed to send CONNECT response: {}", error);
        }
        Ok(())
    }
    async fn connect(&self) -> io::Result<()> {
        debug_assert!(self.packet_len >= MIN_CONNECT_SIZE);
        debug_assert!(self.packet[0..8] == PROTOCOL_ID);

        let mut rpkt = [0; CONNECT_SIZE];
        rpkt[4..8].copy_from_slice(&self.packet[12..16]);
        rpkt[8..16].copy_from_slice(&self.connection_id());

        if let Err(error) = self.socket.send_to(&rpkt, self.addr).await {
            log::error!("failed to send CONNECT response: {}", error);
        }
        Ok(())
    }
    #[inline]
    fn parse_announce(&self) -> Result<(AnnounceParams, Params), Error> {
        debug_assert!(self.packet_len >= MIN_ANNOUNCE_SIZE);
        let info_hash = *array_ref!(self.packet, 16, 20);
        let peer_id = *array_ref!(self.packet, 36, 20);
        let downloaded = i64::from_be_bytes(*array_ref!(self.packet, 56, 8));
        let left = i64::from_be_bytes(*array_ref!(self.packet, 64, 8));
        let uploaded = i64::from_be_bytes(*array_ref!(self.packet, 72, 8));
        let event = i32::from_be_bytes(*array_ref!(self.packet, 80, 4));
        let ip = *array_ref!(self.packet, 84, 4);
        let key = u32::from_be_bytes(*array_ref!(self.packet, 88, 4));
        let num_want = i32::from_be_bytes(*array_ref!(self.packet, 92, 4));
        let port = u16::from_be_bytes(*array_ref!(self.packet, 96, 2));
        let announce_params = AnnounceParams::new(
            info_hash,
            peer_id,
            port,
            self.remote_ip,
            if ip != [0; 4] { Some(ip.into()) } else { None },
            uploaded,
            downloaded,
            left,
            match event {
                0 => Event::None,
                1 => Event::Completed,
                2 => Event::Started,
                3 => Event::Stopped,
                4 => Event::Paused,
                _ => Event::None,
            },
            num_want,
            Some(key),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
        let params = parse_extensions(
            self.tracker.get_params_parser(),
            &self.packet[98..self.packet_len],
        )?;
        Ok((announce_params, params))
    }
    async fn announce(&self) -> Result<(), Error> {
        let (params, ext_params) = self.parse_announce()?;
        let (seeders, leechers, addrs) = self.tracker.announce(params, ext_params).await?;

        let mut rpkt = [0u8; ANNOUNCE_SIZE];
        // action ANNOUNCE
        rpkt[3] = 0x01;
        // transaction_id
        rpkt[4..8].copy_from_slice(&self.packet[12..16]);
        // interval
        rpkt[8..12].copy_from_slice(&self.tracker.get_interval().to_be_bytes());
        rpkt[12..16].copy_from_slice(&leechers.to_be_bytes());
        rpkt[16..20].copy_from_slice(&seeders.to_be_bytes());

        let mut offset = 20;
        for (ip, port) in addrs {
            if self.remote_ip.is_ipv6() {
                rpkt[offset..offset + 16].copy_from_slice(
                    &match ip {
                        IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped(),
                        IpAddr::V6(ipv6) => ipv6,
                    }
                    .octets(),
                );
                rpkt[offset + 16..offset + 18].copy_from_slice(&port.to_be_bytes());
                offset += 18;
            } else {
                rpkt[offset..offset + 4].copy_from_slice(
                    &match ip {
                        IpAddr::V4(ipv4) => ipv4,
                        IpAddr::V6(ipv6) => ipv6.to_ipv4().unwrap(),
                    }
                    .octets(),
                );
                rpkt[offset + 4..offset + 6].copy_from_slice(&port.to_be_bytes());
                offset += 6;
            }
        }
        if let Err(error) = self.socket.send_to(&rpkt[..offset], self.addr).await {
            log::error!("failed to send ANNOUNCE response: {}", error);
        }
        Ok(())
    }
    async fn scrape(&self) -> io::Result<()> {
        let mut rpkt = [0u8; SCRAPE_SIZE];
        // action SCRAPE
        rpkt[3] = 0x02;
        // transaction_id
        rpkt[4..8].copy_from_slice(&self.packet[12..16]);

        let len = (self.packet_len - 16) / 20 * 20 + 16;

        let swarms = self
            .tracker
            .scrape(
                self.packet[16..len]
                    .chunks(20)
                    .map(|s| array_ref!(s, 0, 20)),
            )
            .await;

        for (index, (complete, incomplete, downloaded)) in swarms.iter().enumerate() {
            rpkt[index * 12 + 8..index * 12 + 12].copy_from_slice(&complete.to_be_bytes());
            rpkt[index * 12 + 12..index * 12 + 16].copy_from_slice(&downloaded.to_be_bytes());
            rpkt[index * 12 + 16..index * 12 + 20].copy_from_slice(&incomplete.to_be_bytes());
        }

        if let Err(err) = self
            .socket
            .send_to(&rpkt[..16 + swarms.len() * 12], self.addr)
            .await
        {
            log::error!("failed to send SCRAPE response: {}", err);
        }
        Ok(())
    }
}
