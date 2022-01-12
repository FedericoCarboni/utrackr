use std::{collections::HashMap, marker::PhantomData, net::IpAddr};

use tokio::sync::RwLock;

use super::{
    announce::AnnounceParams,
    config::TrackerConfig,
    extensions::{NoExtension, TrackerExtension},
    params::{EmptyParamsParser, ParamsParser},
    swarm::{Event, Swarm},
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

#[derive(Debug)]
pub struct Tracker<Extension = NoExtension, Config = (), Params = (), P = EmptyParamsParser>
where
    Extension: TrackerExtension<Config, Params, P>,
    Config: Default + Sync + Send,
    Params: Sync + Send,
    P: ParamsParser<Params> + Sync + Send,
{
    extension: Extension,
    config: TrackerConfig<Config>,
    swarms: RwLock<HashMap<[u8; 20], RwLock<Swarm>>>,
    _marker: PhantomData<(Params, P)>,
}

impl<Extension, Config, Params, P> Tracker<Extension, Config, Params, P>
where
    Extension: TrackerExtension<Config, Params, P>,
    Config: Default + Sync + Send,
    Params: Sync + Send,
    P: ParamsParser<Params> + Sync + Send,
{
    #[inline]
    pub fn new(extension: Extension, config: TrackerConfig<Config>) -> Self {
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
                    if ip != peer.ip
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
                            params.left() == 0,
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
}
