use std::{
    collections::HashMap,
    marker::PhantomData,
    net::IpAddr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::sync::RwLock;

use super::{
    announce::AnnounceParams,
    config::TrackerConfig,
    extensions::{NoExtension, TrackerExtension},
    params::{EmptyParamsParser, ParamsParser},
    swarm::{Event, Peer, Swarm},
    Error,
};

#[inline]
fn is_local(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4.is_private(),
        // is_unique_local is not stabilized yet
        IpAddr::V6(ipv6) => (ipv6.segments()[0] & 0xfe00) == 0xfc00,
    }
}

#[inline]
fn match_ip(ip: &IpAddr, peer: &Peer) -> bool {
    match ip {
        IpAddr::V4(a) => peer.ipv4.map(|b| *a == b).unwrap_or(false),
        IpAddr::V6(a) => *a == peer.ipv6,
    }
}

#[derive(Debug)]
pub struct Tracker<Extension = NoExtension, Params = (), P = EmptyParamsParser>
where
    Extension: TrackerExtension<Params, P>,
    Params: Sync + Send,
    P: ParamsParser<Params> + Sync + Send,
{
    extension: Extension,
    config: TrackerConfig,
    swarms: RwLock<HashMap<[u8; 20], RwLock<Swarm>>>,
    _marker: PhantomData<(Params, P)>,
}

impl Tracker {
    #[inline]
    pub fn new(config: TrackerConfig) -> Self {
        Self::with_extension(NoExtension, config)
    }
}

impl<Extension, Params, P> Tracker<Extension, Params, P>
where
    Extension: TrackerExtension<Params, P>,
    Params: Sync + Send,
    P: ParamsParser<Params> + Sync + Send,
{
    #[inline]
    pub fn with_extension(extension: Extension, config: TrackerConfig) -> Self {
        Self {
            extension,
            config,
            swarms: Default::default(),
            _marker: PhantomData,
        }
    }

    #[inline]
    pub fn get_params_parser(&self) -> P {
        self.extension.get_params_parser()
    }

    #[inline]
    pub fn get_interval(&self) -> i32 {
        self.config.interval
    }

    /// Returns `true` if the tracker should accept the peer's self-declared IP
    /// address.
    #[inline]
    fn is_trusted(&self, remote_ip: &IpAddr) -> bool {
        self.config.trust_ip_param_if_local && is_local(remote_ip)
            || self.config.unsafe_trust_ip_param
    }

    pub async fn announce(
        &self,
        params: AnnounceParams,
        ext_params: Params,
    ) -> Result<(i32, i32, Vec<(IpAddr, u16)>), Error> {
        // No reasonable BitTorrent client should ever listen for peer
        // connections on system ports (1-1023). We refuse the announce request
        // immediately to avoid being part of a DDOS attack. Of course 0 is not
        // a valid port so it's discarded as well.
        if params.port() < 1024 {
            return Err(Error::InvalidPort);
        }

        let ip = params
            .unsafe_ip()
            .filter(|_| self.is_trusted(&params.remote_ip()))
            .unwrap_or_else(|| params.remote_ip());

        let swarms = self.swarms.read().await;

        if let Some(swarm) = swarms.get(params.info_hash()) {
            let result = {
                let swarm = swarm.read().await;
                let peer = swarm.peers().get(params.peer_id());
                let mut peerlist = true;
                if let Some(peer) = peer {
                    // If the peer_id is already in the swarm check that the IP or
                    // key match. Announce requests will be rejected if IP address
                    // changed and the key doesn't match or is absent.
                    if !match_ip(&ip, peer)
                        && (self.config.deny_all_ip_changes
                            || params.key().is_none()
                            || params.key() != peer.key)
                    {
                        return Err(Error::IpAddressChanged);
                    }
                    // If the peer announced too soon, don't return any peers
                    if params.time() - peer.last_announce < self.config.min_interval as u64 {
                        peerlist = false;
                    }
                }
                // Allow extensions to run custom validation on the parameters and
                // peer.
                self.extension.validate(&params, &ext_params, peer)?;
                // Select the peers if
                let peers =
                    if peerlist && params.num_want() != 0 && params.event() != Event::Stopped {
                        swarm.select(
                            params.peer_id(),
                            &ip,
                            params.left() == 0 || params.event() == Event::Paused,
                            if params.num_want() < 0 {
                                self.config.default_num_want
                            } else if params.num_want() > self.config.max_num_want {
                                self.config.max_num_want
                            } else {
                                params.num_want()
                            } as usize,
                        )
                    } else {
                        vec![]
                    };
                Ok((swarm.complete(), swarm.incomplete(), peers))
            };
            let mut swarm = swarm.write().await;
            swarm.announce(&params, ip);
            result
        } else if self.config.track_unknown_torrents {
            drop(swarms); // drop the read guard, we need a write one
            self.extension.validate(&params, &ext_params, None)?;

            let mut swarm = Swarm::default();
            swarm.announce(&params, ip);
            let mut swarms = self.swarms.write().await;
            swarms.insert(*params.info_hash(), RwLock::new(swarm));
            Ok((0, 0, vec![]))
        } else {
            Err(Error::TorrentNotFound)
        }
    }

    pub async fn scrape(
        &self,
        info_hashes: impl Iterator<Item = &[u8; 20]>,
    ) -> Vec<(i32, i32, i32)> {
        let mut v = Vec::with_capacity(info_hashes.size_hint().1.unwrap_or(1));
        let swarms = self.swarms.read().await;
        for info_hash in info_hashes {
            if let Some(swarm) = swarms.get(info_hash) {
                let swarm = swarm.read().await;
                v.push((swarm.complete(), swarm.incomplete(), swarm.downloaded()));
            } else {
                v.push((0, 0, 0));
            }
        }
        v
    }

    pub async fn run_clean_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let swarms = self.swarms.write().await;
            for (_, swarm) in swarms.iter() {
                let mut swarm = swarm.write().await;
                // TODO: swarms themselves should be removed as well if they
                // have to peers
                swarm.evict(now, self.config.max_interval as u64);
            }
        }
    }
}
