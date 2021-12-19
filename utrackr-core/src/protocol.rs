use std::{
    io,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use log::{debug, error, info};
use tokio::{net::UdpSocket, time};

// the biggest packet is announce 98 bytes (or 620 bytes with extensions, but
// they're currently not supported)
// https://www.libtorrent.org/udp_tracker_protocol.html#extensions
const MAX_PKT_SIZE: usize = 98;
const MIN_PKT_SIZE: usize = 16;
const MIN_CONNECT_SIZE: usize = 16;
const MIN_ANNOUNCE_SIZE: usize = 98;
const MIN_SCRAPE_SIZE: usize = 36;

const PROTOCOL_ID: &'static [u8] = &0x41727101980i64.to_be_bytes();

const ACTION_CONNECT: &'static [u8] = &0x0i32.to_be_bytes();
const ACTION_ANNOUNCE: &'static [u8] = &0x1i32.to_be_bytes();
const ACTION_SCRAPE: &'static [u8] = &0x2i32.to_be_bytes();
const ACTION_ERROR: &'static [u8] = &0x3i32.to_be_bytes();

pub struct UdpTracker {
    sock: UdpSocket,
    // the secret on which the connection_id is generated
    // 64 bits of entropy are enough for a 64 bit id
    secret: [u8; 8],
}

impl UdpTracker {
    pub async fn run_forever(&'static self) -> io::Result<()> {
        loop {
            let mut buf = [0u8; MAX_PKT_SIZE];
            match self.sock.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    // ignore packets that are too short to be part of the
                    // udp tracker protocol
                    if len < MIN_PKT_SIZE {
                        continue;
                    }
                    // handle the request concurrently
                    tokio::spawn(async move {
                        self.accept_packet(&buf[..len], addr).await;
                    });
                }
                Err(err) => {
                    error!("{}", err);
                    info!("waiting 60 seconds before retrying");
                    time::sleep(Duration::from_secs(60)).await;
                }
            }
        }
    }
    async fn accept_packet(&self, pkt: &[u8], addr: SocketAddr) {
        match &pkt[8..12] {
            ACTION_CONNECT if pkt.len() >= MIN_CONNECT_SIZE && &pkt[0..8] == PROTOCOL_ID => {
                // connect packet
            }
            ACTION_ANNOUNCE if pkt.len() >= MIN_ANNOUNCE_SIZE => {
                // announce packet
            }
            ACTION_SCRAPE if pkt.len() >= MIN_SCRAPE_SIZE => {
                // scrape packet
            }
            _ => {}
        }
    }
    /// Generates the connection_id for this address
    fn connection_id(&self, addr: SocketAddr) -> [u8; 8] {
        let mut digest = crc64fast::Digest::new();
        // update the digest with the secret, this will make potential attackers
        // unable to spoof connection ids without knowing the secret
        digest.write(&self.secret);
        // update the digest with ip and port, the connection_id
        // will be valid only for the same ip address
        digest.write(
            &match addr.ip() {
                IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped(),
                IpAddr::V6(ipv6) => ipv6,
            }
            .octets(),
        );
        digest.write(&addr.port().to_ne_bytes());
        digest.sum64().to_ne_bytes()
    }
    async fn connect(&self, pkt: &[u8], addr: SocketAddr) {

    }
}
