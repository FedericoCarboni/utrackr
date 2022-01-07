use std::{
    fmt, io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use arrayref::array_ref;
use ring::digest;
use tokio::net::UdpSocket;

use crate::core::{Announce, Error, Event, Tracker};
use crate::udp::protocol::{
    Secret, ACTION_ANNOUNCE, ACTION_CONNECT, ACTION_SCRAPE, ANNOUNCE_SIZE, CONNECT_SIZE,
    MAX_PACKET_SIZE, MIN_ANNOUNCE_SIZE, MIN_CONNECT_SIZE, MIN_PACKET_SIZE, MIN_SCRAPE_SIZE,
    PROTOCOL_ID, SCRAPE_SIZE,MAX_SCRAPE_TORRENTS
};

/// Turns IPv6 addressed mapped to IPv4 addresses to IPv4 addresses. Does nothing
/// to IPv4 addresses or proper IPv6 addresses. This is needed to support both
/// IPv4 and IPv6 at the same time.
#[inline]
const fn to_canonical_ip(ip: IpAddr) -> IpAddr {
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
    secret: Secret,
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
            .field("secret", &"[secret]")
            .field("packet", &&self.packet[..self.packet_len])
            .field("addr", &self.addr)
            .finish()
    }
}

impl Transaction {
    pub(crate) fn new(
        socket: Arc<UdpSocket>,
        secret: Secret,
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
                self.announce().await?;
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

    fn verify_connection_id(&self) -> bool {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        // verify_connection_id(
        //     &self.secret,
        //     timestamp / 120,
        //     &match self.addr.ip() {
        //         IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped(),
        //         IpAddr::V6(ipv6) => ipv6,
        //     }
        //     .octets(),
        //     array_ref!(self.packet, 0, 8),
        // )
        true
    }

    fn connection_id(&self) -> [u8; 8] {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        // make_connection_id(
        //     &self.secret,
        //     &(timestamp / 120).to_be_bytes(),
        //     &match self.addr.ip() {
        //         IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped(),
        //         IpAddr::V6(ipv6) => ipv6,
        //     }
        //     .octets(),
        // )
        [0; 8]
    }

    #[inline]
    fn ip(&self) -> IpAddr {
        to_canonical_ip(self.addr.ip())
    }

    /// Sends an error packet to the requesting client.
    /// We don't make any assumptions about clients, so all error messages
    /// should be printable ASCII characters.
    async fn error(&self, message: &str) -> io::Result<()> {
        // make sure that we have a terminating 0 byte
        debug_assert!(message.len() <= 55, "error message too long");
        // make sure that the error message contains only printable ascii chars
        debug_assert!(
            message.bytes().any(|b| !(0x20..=0x7E).contains(&b)),
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
    fn parse_announce(&self) -> Announce {
        debug_assert!(self.packet_len >= MIN_ANNOUNCE_SIZE);
        let info_hash = *array_ref!(self.packet, 16, 20);
        let peer_id = *array_ref!(self.packet, 36, 20);
        let downloaded = i64::from_be_bytes(*array_ref!(self.packet, 56, 8));
        let left = i64::from_be_bytes(*array_ref!(self.packet, 64, 8));
        let uploaded = i64::from_be_bytes(*array_ref!(self.packet, 72, 8));
        let event = i32::from_be_bytes(*array_ref!(self.packet, 80, 4));
        // the ip_address in the announce packet is currently ignored
        // let ip_address = &self.packet[84..88];
        let key = u32::from_be_bytes(*array_ref!(self.packet, 88, 4));
        let num_want = i32::from_be_bytes(*array_ref!(self.packet, 92, 4));
        let port = u16::from_be_bytes(*array_ref!(self.packet, 96, 2));
        Announce {
            info_hash,
            peer_id,
            downloaded,
            uploaded,
            left,
            addr: SocketAddr::new(self.ip(), port),
            key: Some(key),
            event: match event {
                1 => Event::Completed,
                2 => Event::Started,
                3 => Event::Stopped,
                _ => Event::None,
            },
            num_want,
            instant: self.instant,
        }
    }
    async fn announce(&mut self) -> io::Result<()> {
        match self.tracker.announce(self.parse_announce()).await {
            Ok(res) => {
                let (seeders, leechers, addrs) = res
                    .map(|(seeders, leechers, addrs)| {
                        (
                            seeders,
                            leechers,
                            if !addrs.is_empty() { Some(addrs) } else { None },
                        )
                    })
                    .unwrap_or((0, 0, None));

                let mut rpkt = [0u8; ANNOUNCE_SIZE];
                // action ANNOUNCE
                rpkt[3] = 0x01;
                // transaction_id
                rpkt[4..8].copy_from_slice(&self.packet[12..16]);
                // interval
                rpkt[8..12].copy_from_slice(&self.tracker.interval().to_be_bytes());
                rpkt[12..16].copy_from_slice(&leechers.to_be_bytes());
                rpkt[16..20].copy_from_slice(&seeders.to_be_bytes());

                let mut offset = 20;
                if let Some(addrs) = addrs {
                    for (_, addr) in addrs {
                        if self.ip().is_ipv6() {
                            rpkt[offset..offset + 16].copy_from_slice(
                                &match addr.ip() {
                                    IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped(),
                                    IpAddr::V6(ipv6) => ipv6,
                                }
                                .octets(),
                            );
                            rpkt[offset + 16..offset + 18]
                                .copy_from_slice(&addr.port().to_be_bytes());
                            offset += 18;
                        } else {
                            rpkt[offset..offset + 4].copy_from_slice(
                                &match addr.ip() {
                                    IpAddr::V4(ipv4) => ipv4,
                                    IpAddr::V6(ipv6) => ipv6.to_ipv4().unwrap(),
                                }
                                .octets(),
                            );
                            rpkt[offset + 4..offset + 6]
                                .copy_from_slice(&addr.port().to_be_bytes());
                            offset += 6;
                        }
                    }
                }
                if let Err(error) = self.socket.send_to(&rpkt[..offset], self.addr).await {
                    log::error!("failed to send ANNOUNCE response: {}", error);
                }
            }
            // the tracker rejected the announce request with an error
            Err(err) => self.error(err.message()).await?,
        }
        Ok(())
    }
    async fn scrape(&mut self) -> io::Result<()> {
        let mut rpkt = [0u8; SCRAPE_SIZE];
        // action SCRAPE
        rpkt[3] = 0x02;
        // transaction_id
        rpkt[4..8].copy_from_slice(&self.packet[12..16]);

        let mut info_hashes = [[0; 20]; MAX_SCRAPE_TORRENTS];
        let mut i = 0;
        let mut offset = 16;
        while offset < self.packet_len {
            let info_hash = *array_ref!(self.packet, offset, 20);
            if info_hash == [0; 20] {
                break;
            }
            info_hashes[i] = info_hash;
            offset += 20;
            i += 1;
        }

        let mut offset = 8;
        let torrents = self.tracker.scrape(&info_hashes[..i]).await;
        for (seeders, leechers, completed) in torrents {
            rpkt[offset..offset + 4].copy_from_slice(&seeders.to_be_bytes());
            rpkt[offset + 4..offset + 8].copy_from_slice(&completed.to_be_bytes());
            rpkt[offset + 8..offset + 12].copy_from_slice(&leechers.to_be_bytes());
            offset += 12;
        }

        if let Err(err) = self.socket.send_to(&rpkt[..offset], self.addr).await {
            log::error!("failed to send SCRAPE response: {}", err);
        }
        Ok(())
    }
}
