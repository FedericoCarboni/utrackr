use std::{collections::HashMap, net::SocketAddr, time::Instant};

use rand::seq::IteratorRandom;

use crate::config::TrackerConfig;
use crate::swarm::{Event, Peer, Swarm};

pub enum AnnounceError {
    UnknownTorrent,
    InvalidPort,
    InvalidKey,
    InvalidNumWant,
}

#[derive(Debug)]
pub struct AnnounceRequest {
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
    pub key: Option<u32>,
    pub addr: SocketAddr,
    pub event: Event,
    pub downloaded: u64,
    pub uploaded: u64,
    pub left: u64,
    pub num_want: i32,
    pub timestamp: Instant,
}

#[derive(Debug)]
pub struct Tracker {
    swarms: HashMap<[u8; 20], Swarm>,
    config: TrackerConfig,
}

impl Default for Tracker {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl Tracker {
    pub fn new(config: TrackerConfig) -> Self {
        Self {
            swarms: HashMap::new(),
            config,
        }
    }
    fn validate_announce(
        &self,
        req: &AnnounceRequest,
        swarm: Option<&Swarm>,
    ) -> Result<(), AnnounceError> {
        // Ports 1-1023 are system ports, no reasonable BitTorrent client should
        // ever listen for peer connections on those ports, so we refuse the
        // ANNOUNCE request to avoid being exploited for a DDOS attack.
        if req.addr.port() < 1024 {
            return Err(AnnounceError::InvalidPort);
        }
        // Prevent clients from requesting too many peers
        if req.num_want > self.config.max_numwant {
            return Err(AnnounceError::InvalidNumWant);
        }
        swarm
            .and_then(|swarm| swarm.peers().get(&req.peer_id))
            .map(|peer| {
                if peer.key == req.key {
                    Ok(())
                } else {
                    Err(AnnounceError::InvalidKey)
                }
            })
            .unwrap_or(Ok(()))?;
        Ok(())
    }
    pub fn start_announce(
        &self,
        req: &AnnounceRequest,
    ) -> Result<Option<Vec<(&[u8; 20], &Peer)>>, AnnounceError> {
        let swarm = self.swarms.get(&req.info_hash);
        self.validate_announce(req, swarm)?;
        let num_want = if req.num_want < 0 {
            self.config.default_numwant
        } else if req.num_want > self.config.max_numwant {
            self.config.max_numwant
        } else {
            req.num_want
        } as usize;
        swarm
            .map(|swarm| {
                Ok(Some(
                    swarm
                        .peers()
                        .iter()
                        .filter(|(&id, peer)| {
                            id != req.peer_id
                                && peer.key != req.key
                                && (req.addr.is_ipv6() || peer.addr.is_ipv4())
                                && (req.left != 0 || peer.left > 0)
                        })
                        .choose_multiple(&mut rand::thread_rng(), num_want),
                ))
            })
            .unwrap_or_else(|| {
                if self.config.enable_unknown_torrents {
                    Ok(None)
                } else {
                    Err(AnnounceError::UnknownTorrent)
                }
            })
    }
    pub fn announce(&mut self, req: &AnnounceRequest) -> Result<(), AnnounceError> {
        debug_assert!(self.validate_announce(req, self.swarms.get(&req.info_hash)).is_ok());
        if !self.config.enable_unknown_torrents {
            if let Some(swarm) = self.swarms.get_mut(&req.info_hash) {
                swarm.announce(req)?;
                return Ok(());
            } else {
                return Err(AnnounceError::UnknownTorrent);
            }
        }
        self.swarms
            .entry(req.info_hash)
            .or_default()
            .announce(req)?;
        Ok(())
    }
    pub fn scrape<'a>(
        &'a self,
        info_hashes: &'a [[u8; 20]],
    ) -> impl Iterator<Item = Option<&'a Swarm>> {
        info_hashes
            .iter()
            .map(|info_hash| self.swarms.get(info_hash))
    }
    pub fn evict(&mut self) {
        for (_, swarm) in self.swarms.iter_mut() {
            let mut complete = 0;
            let mut incomplete = 0;
            swarm.peers_mut().retain(|_, peer| {
                let is_valid = peer.announce.elapsed().as_secs() <= self.config.max_interval;
                if !is_valid {
                    if peer.left == 0 {
                        complete += 1;
                    } else {
                        incomplete += 1;
                    }
                }
                is_valid
            });
            swarm.complete -= complete;
            swarm.incomplete -= incomplete;
        }
    }
}

// #[cfg(test)]
// mod test {
//     use super::*;
//     use std::net::IpAddr;

//     #[test]
//     fn test_announce() {
//         let mut tracker = Tracker::new(TrackerConfig::default());
//         tracker
//             .announce(
//                 [0u8; 20],
//                 [1u8; 20],
//                 1,
//                 SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 2000),
//                 Event::None,
//                 0,
//                 0,
//                 2300,
//                 0,
//             )
//             .unwrap();
//         tracker
//             .announce(
//                 [0u8; 20],
//                 [2u8; 20],
//                 2,
//                 SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 2000),
//                 Event::None,
//                 0,
//                 0,
//                 0,
//                 0,
//             )
//             .unwrap();
//         tracker
//             .announce(
//                 [0u8; 20],
//                 [3u8; 20],
//                 3,
//                 SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 2000),
//                 Event::None,
//                 0,
//                 0,
//                 0,
//                 0,
//             )
//             .unwrap();
//         println!("{:#?}", tracker);
//         println!(
//             "{:#?}",
//             tracker.select_peers([0u8; 20], [2u8; 20], 2, true, false, 32)
//         );
//     }
// }
