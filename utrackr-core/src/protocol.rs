use std::{
    io,
    net::{IpAddr, SocketAddr},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use log::{debug, error, info};
use tokio::{
    net::{ToSocketAddrs, UdpSocket},
    time,
};

// the biggest packet is announce 98 bytes (or 620 bytes with extensions, but
// they're currently not supported)
// https://www.libtorrent.org/udp_tracker_protocol.html#extensions
const MAX_PKT_SIZE: usize = 98;
const MIN_PKT_SIZE: usize = 16;
const MIN_CONNECT_SIZE: usize = 16;
const MIN_ANNOUNCE_SIZE: usize = 98;
const MIN_SCRAPE_SIZE: usize = 36;
const CONNECT_SIZE: usize = 16;
const ANNOUNCE_SIZE: usize = 2048;

const PROTOCOL_ID: &'static [u8] = &0x41727101980i64.to_be_bytes();

const ACTION_CONNECT: &'static [u8] = &0x0i32.to_be_bytes();
const ACTION_ANNOUNCE: &'static [u8] = &0x1i32.to_be_bytes();
const ACTION_SCRAPE: &'static [u8] = &0x2i32.to_be_bytes();
const ACTION_ERROR: &'static [u8] = &0x3i32.to_be_bytes();

fn generate_connection_id(secret: &[u8; 8], minutes: &[u8], ip: &[u8], port: &[u8]) -> [u8; 8] {
    let mut digest = crc64fast::Digest::new();
    // update the digest with the secret, this will make potential attackers
    // unable to spoof connection ids without knowing the secret
    digest.write(secret);
    // update the digest with the minutes, this will make sure that the connection_id
    // is invalidated at least every 2 minutes
    digest.write(minutes);
    // update the digest with ip and port, the connection_id
    // will be valid only for the same ip address
    digest.write(ip);
    digest.write(port);
    digest.sum64().to_be_bytes()
}

pub struct UdpTracker {
    sock: UdpSocket,
    // the secret on which the connection_id is generated
    // 64 bits of entropy are enough for a 64 bit id
    secret: [u8; 8],
    start: Instant,
}

impl UdpTracker {
    pub async fn bind<T: ToSocketAddrs>(addrs: T) -> io::Result<Self> {
        let mut secret = [0u8; 8];
        openssl::rand::rand_bytes(&mut secret)?;
        Ok(Self {
            sock: UdpSocket::bind(addrs).await?,
            secret,
            start: Instant::now(),
        })
    }
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
                        match self.accept_packet(&buf[..len], addr).await {
                            Err(err) => error!("{}", err),
                            _ => {}
                        }
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
    async fn accept_packet(&self, pkt: &[u8], addr: SocketAddr) -> io::Result<()> {
        match &pkt[8..12] {
            ACTION_CONNECT if pkt.len() >= MIN_CONNECT_SIZE && &pkt[0..8] == PROTOCOL_ID => {
                // connect packet
                self.connect(pkt, addr).await?;
            }
            ACTION_ANNOUNCE if pkt.len() >= MIN_ANNOUNCE_SIZE => {
                // announce packet
                self.announce(pkt, addr).await?;
            }
            ACTION_SCRAPE if pkt.len() >= MIN_SCRAPE_SIZE => {
                // scrape packet
            
            }
            _ => {}
        };
        Ok(())
    }
    fn check_connection_id(&self, addr: SocketAddr, connection_id: &[u8]) -> bool {
        let minutes = self.start.elapsed().as_secs() / 60;
        let ip = match addr.ip() {
            IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped(),
            IpAddr::V6(ipv6) => ipv6,
        }
        .octets();
        let port = addr.port().to_be_bytes();
        connection_id == generate_connection_id(&self.secret, &minutes.to_be_bytes(), &ip, &port)
            || connection_id
                == generate_connection_id(&self.secret, &(minutes - 1).to_be_bytes(), &ip, &port)
    }
    /// Generates the connection_id for this address
    fn connection_id(&self, addr: SocketAddr) -> [u8; 8] {
        let minutes = self.start.elapsed().as_secs() / 60;
        generate_connection_id(
            &self.secret,
            &minutes.to_be_bytes(),
            &match addr.ip() {
                IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped(),
                IpAddr::V6(ipv6) => ipv6,
            }
            .octets(),
            &addr.port().to_be_bytes(),
        )
    }
    async fn connect(&self, pkt: &[u8], addr: SocketAddr) -> io::Result<()> {
        let mut rpkt = [0u8; CONNECT_SIZE];
        rpkt[0..4].copy_from_slice(&ACTION_CONNECT);
        rpkt[4..8].copy_from_slice(&pkt[12..16]);
        rpkt[8..16].copy_from_slice(&self.connection_id(addr));

        self.sock.send_to(&rpkt, addr).await?;
        Ok(())
    }
    async fn announce(&self, pkt: &[u8], addr: SocketAddr) -> io::Result<()> {
        if !self.check_connection_id(addr, &pkt[0..8]) {
            // deny access if the connection_id is invalid
            let mut rpkt = [0u8; 22];
            rpkt[0..4].copy_from_slice(ACTION_ERROR);
            rpkt[4..8].copy_from_slice(&pkt[12..16]);
            rpkt[8..22].copy_from_slice(b"access denied\0");
            self.sock.send_to(&rpkt, addr).await?;
            return Ok(());
        }
        let mut rpkt = [0u8; ANNOUNCE_SIZE];
        rpkt[0..4].copy_from_slice(ACTION_ANNOUNCE);
        // copy transaction_id, we don't need to know what's inside it
        // we just have to forward it to the client
        rpkt[4..8].copy_from_slice(&pkt[12..16]);
        // rpkt[8..12].copy_from_slice();
        Ok(())
    }
}
