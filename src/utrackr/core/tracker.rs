use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
};

use tokio::sync::RwLock;

use super::{
    announce::AnnounceParams,
    config::TrackerConfig,
    extensions::TrackerExtension,
    params::{EmptyParamsParser, ParamsParser},
    swarm::Swarm,
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
fn is_global(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            if u32::from_be_bytes(ipv4.octets()) == 0xc0000009
                || u32::from_be_bytes(ipv4.octets()) == 0xc000000a
            {
                return true;
            }
            !ipv4.is_private()
            && !ipv4.is_loopback()
            && !ipv4.is_link_local()
            && !ipv4.is_broadcast()
            && !ipv4.is_documentation()
            && !(ipv4.octets()[0] == 100 && (ipv4.octets()[1] & 0b1100_0000 == 0b0100_0000))
            // addresses reserved for future protocols (`192.0.0.0/24`)
            && !(ipv4.octets()[0] == 192 && ipv4.octets()[1] == 0 && ipv4.octets()[2] == 0)
            && !(ipv4.octets()[0] & 240 == 240 && !ipv4.is_broadcast())
            && !(ipv4.octets()[0] == 198 && (ipv4.octets()[1] & 0xfe) == 18)
            // Make sure the address is not in 0.0.0.0/8
            && ipv4.octets()[0] != 0
        }
        IpAddr::V6(ipv6) => {
            !ipv6.is_multicast()
                && !ipv6.is_loopback()
                && !((ipv6.segments()[0] & 0xffc0) == 0xfe80)
                && !((ipv6.segments()[0] & 0xfe00) == 0xfc00)
                && !ipv6.is_unspecified()
                && !((ipv6.segments()[0] == 0x2001) && (ipv6.segments()[1] == 0xdb8))
        }
    }
}

pub trait AnnounceWriter {
    fn counts(&mut self, complete: i32, incomplete: i32);
    fn peer(&mut self, peer_id: &[u8; 20], addr: &SocketAddr);
}

#[derive(Debug)]
pub struct Tracker<Extension, Config = (), Params = (), P = EmptyParamsParser>
where
    Extension: TrackerExtension<Config, Params, P>,
    Config: Default,
    P: ParamsParser<Params>,
{
    extension: Extension,
    config: TrackerConfig<Config>,
    swarms: RwLock<HashMap<[u8; 20], RwLock<Swarm>>>,
}

impl<Extension, Config, Params, P> Tracker<Extension, Config, Params, P>
where
    Extension: TrackerExtension<Config, Params, P>,
    Config: Default,
    P: ParamsParser<Params>,
{
    #[inline]
    pub fn new(extension: Extension, config: TrackerConfig<Config>) -> Self {
        Self {
            extension,
            config,
            swarms: Default::default(),
        }
    }

    #[inline]
    pub fn get_params_parser(&self) -> P {
        self.extension.get_params_parser()
    }

    /// Returns `true` if the tracker should accept the peer's self-declared IP
    /// address.
    #[inline]
    fn is_trusted(&self, remote_ip: &IpAddr) -> bool {
        self.config.trust_ip_param_if_local && is_local(remote_ip)
            || self.config.unsafe_trust_ip_param
    }

    pub async fn announce<W: AnnounceWriter>(
        &self,
        params: AnnounceParams<Params>,
        w: &mut W,
    ) -> Result<(), Error> {
        // No reasonable BitTorrent client should ever listen for peer
        // connections on system ports (1-1023). We refuse the announce request
        // immediately to avoid being part of a DDOS attack. Of course 0 is not
        // a valid port so it's discarded as well.
        if params.port() < 1024 {
            return Err(Error::InvalidPort);
        }

        let ip = *params
            .unsafe_ip()
            .filter(|_| self.is_trusted(params.remote_ip()))
            .unwrap_or_else(|| params.remote_ip());

        let swarms = self.swarms.read().await;

        if let Some(swarm) = swarms.get(params.info_hash()) {
            let swarm = swarm.read().await;
            let peer = swarm.peers().get(params.peer_id());
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
            }
            // Allow extensions to run custom validation on the parameters and
            // peer.
            self.extension.validate(&params, peer)?;
            // Write peer counts
            w.counts(swarm.complete(), swarm.incomplete());
        } else if self.config.track_unknown_torrents {
            drop(swarms); // drop the read guard, we need a write one
            let swarms = self.swarms.write().await;
            let swarm = Swarm::default();
            swarm.announce(&params, ip);
            swarms.insert(*params.info_hash(), RwLock::new(swarm));
        } else {
            return Err(Error::TorrentNotFound);
        }

        Ok(())
    }
}
