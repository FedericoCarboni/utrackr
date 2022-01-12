//! UDP Tracker Protocol implemented according to BEP 15[^1], includes support
//! for UDP extensions as specified by BEP 41[^2].
//!
//! `libtorrent-rasterbar`'s implementation of those extensions is based on
//! Arvid Norberg's specification[^3], which differs enough from BEP 41[^2]
//! to make the two incompatible to some extent. Fortunately as long as the
//! client doesn't include the authentication extension[^4] in the request, the
//! tracker will behave as expected.
//!
//! ## Limitations
//! The tracker can't read request strings (path and query components) of more
//! than `1934` characters. Realistically path and query together should not
//! exceed `255` as most client implementations will only send up to `255`
//! characters[^5].
//!
//! BEP 41 is not widely implemented, so it may not work for all BitTorrent clients.
//!
//! [^1]: [BEP 15, UDP Tracker Protocol for BitTorrent](https://www.bittorrent.org/beps/bep_0015.html)
//!
//! [^2]: [BEP 41, UDP Tracker Protocol Extensions](https://www.bittorrent.org/beps/bep_0041.html)
//!
//! [^3]: [Arvid Norberg's specification for `libtorrent-rasterbar` § Extensions](https://www.libtorrent.org/udp_tracker_protocol.html#extensions)
//!
//! [^4]: [Arvid Norberg's specification for `libtorrent-rasterbar` § Authentication](https://www.libtorrent.org/udp_tracker_protocol.html#authentication)
//!
//! [^5]: [`libtorrent-rasterbar` only sends the first 255 chars of the request string](https://github.com/arvidn/libtorrent/blob/RC_2_0/src/udp_tracker_connection.cpp#L743)

use std::{
    io,
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};

use rand::random;
use tokio::net::UdpSocket;

use crate::core::{
    extensions::{TrackerExtension, NoExtension},
    params::{EmptyParamsParser, ParamsParser},
    Tracker, UdpConfig,
};
use crate::udp::protocol::{Secret, Transaction, MAX_PACKET_SIZE, MIN_PACKET_SIZE};

mod extensions;
mod protocol;

pub struct UdpTracker<Extension = NoExtension, Config = (), Params = (), P = EmptyParamsParser>
where
    Extension: TrackerExtension<Config, Params, P>,
    Config: Default + Sync + Send,
    Params: Sync + Send,
    P: ParamsParser<Params> + Sync + Send,
{
    tracker: Arc<Tracker<Extension, Config, Params, P>>,
    socket: Arc<UdpSocket>,
    secret: Secret,
}

impl<Extension, Config, Params, P> UdpTracker<Extension, Config, Params, P>
where
    Extension: 'static + TrackerExtension<Config, Params, P> + Sync + Send,
    Config: 'static + Default + Sync + Send,
    Params: 'static + Sync + Send,
    P: 'static + ParamsParser<Params> + Sync + Send,
{
    pub async fn bind(
        tracker: Arc<Tracker<Extension, Config, Params, P>>,
        config: UdpConfig,
    ) -> io::Result<Self> {
        let socket = UdpSocket::bind(config.bind.addrs()).await?;
        let addr = socket.local_addr()?;
        log::info!("udp tracker bound to {:?}", addr);
        let secret = random();
        Ok(Self {
            socket: Arc::new(socket),
            secret,
            tracker,
        })
    }
    /// Run the server indefinitely, this function is cancel safe.
    pub async fn run(self) {
        loop {
            let mut packet = [0; MAX_PACKET_SIZE];
            match self.socket.recv_from(&mut packet).await {
                Ok((packet_len, addr)) => {
                    // ill-sized packets are ignored
                    if packet_len < MIN_PACKET_SIZE {
                        log::trace!("packet too small: received packet of length {}", packet_len,);
                        continue;
                    }
                    if packet_len > MAX_PACKET_SIZE {
                        log::trace!(
                            "packet too big: received packet of length {}, ignored",
                            packet_len,
                        );
                        continue;
                    }
                    log::trace!("received packet of length {}", packet_len);
                    let socket = Arc::clone(&self.socket);
                    let secret = self.secret;
                    let tracker = Arc::clone(&self.tracker);
                    let remote_ip = match addr.ip() {
                        ipv4 @ IpAddr::V4(_) => ipv4,
                        ipv6 @ IpAddr::V6(v6) => match v6.octets() {
                            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, a, b, c, d] => {
                                IpAddr::V4(Ipv4Addr::new(a, b, c, d))
                            }
                            _ => ipv6,
                        },
                    };
                    //let instant = Instant::now();
                    // handle the request concurrently
                    tokio::spawn(async move {
                        let transaction = Transaction {
                            socket,
                            secret,
                            tracker,
                            remote_ip,
                            packet,
                            packet_len,
                            addr,
                        };
                        if let Err(err) = transaction.handle().await {
                            log::error!("transaction handler failed: {}", err);
                        }
                    });
                }
                Err(err) => {
                    log::error!("unexpected io error while reading udp socket {}", err);
                }
            }
        }
    }
}
