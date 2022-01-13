use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

use rand::seq::IteratorRandom;

use crate::core::announce::AnnounceParams;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Event {
    None,
    Completed,
    Started,
    Stopped,
    Paused,
}

#[derive(Debug)]
pub struct Peer {
    pub downloaded: i64,
    pub uploaded: i64,
    pub left: i64,
    pub is_partial_seeder: bool,
    pub ipv4: Option<Ipv4Addr>,
    pub ipv6: Ipv6Addr,
    pub port: u16,
    pub key: Option<u32>,
    pub last_announce: u64,
}

impl Peer {
    #[inline]
    pub fn is_seeder(&self) -> bool {
        self.left == 0 || self.is_partial_seeder
    }
}

/// In-Memory store of a peer swarm
#[derive(Debug, Default)]
pub struct Swarm {
    complete: i32,
    incomplete: i32,
    downloaded: i32,
    peers: BTreeMap<[u8; 20], Peer>,
}

impl Swarm {
    #[inline]
    pub fn complete(&self) -> i32 {
        self.complete
    }
    #[inline]
    pub fn incomplete(&self) -> i32 {
        self.incomplete
    }
    #[inline]
    pub fn downloaded(&self) -> i32 {
        self.downloaded
    }
    #[inline]
    pub fn peers(&self) -> &BTreeMap<[u8; 20], Peer> {
        &self.peers
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }
    pub fn select(
        &self,
        peer_id: &[u8; 20],
        ip: &IpAddr,
        seeding: bool,
        amount: usize,
    ) -> Vec<(IpAddr, u16)> {
        self.peers
            .iter()
            .filter_map(|(id, peer)| {
                // don't announce peers to themselves
                if id != peer_id
                    // don't announce seeders to other seeders
                    && (peer.is_seeder() || !seeding)
                {
                    if ip.is_ipv4() {
                        peer.ipv4.map(|ipv4| (IpAddr::V4(ipv4), peer.port))
                    } else {
                        Some((IpAddr::V6(peer.ipv6), peer.port))
                    }
                } else {
                    None
                }
            })
            .choose_multiple(&mut rand::thread_rng(), amount)
    }
    pub fn announce(&mut self, params: &AnnounceParams, ip: IpAddr) {
        match params.event() {
            Event::Completed => {
                self.downloaded += 1;
            }
            Event::Stopped => {
                if let Some(peer) = self.peers.remove(params.peer_id()) {
                    if peer.is_seeder() {
                        self.complete -= 1;
                    } else {
                        self.incomplete -= 1;
                    }
                }
                return;
            }
            _ => {}
        }
        if let Some(peer) = self.peers.get_mut(params.peer_id()) {
            peer.downloaded = params.downloaded();
            peer.uploaded = params.uploaded();
            peer.left = params.left();
            if params.event() == Event::Paused {
                peer.is_partial_seeder = true;
            }
            match ip {
                IpAddr::V4(ipv4) => peer.ipv4 = Some(ipv4),
                IpAddr::V6(ipv6) => peer.ipv6 = ipv6,
            }
            peer.port = params.port();
            peer.key = params.key();
            peer.last_announce = params.time();
        } else {
            if params.left() == 0 {
                self.complete += 1;
            } else {
                self.incomplete += 1;
            }
            self.peers.insert(
                *params.peer_id(),
                Peer {
                    downloaded: params.downloaded(),
                    uploaded: params.uploaded(),
                    left: params.left(),
                    is_partial_seeder: params.event() == Event::Paused,
                    ipv4: match ip {
                        IpAddr::V4(ipv4) => Some(ipv4),
                        IpAddr::V6(_) => None,
                    },
                    ipv6: match ip {
                        IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped(),
                        IpAddr::V6(ipv6) => ipv6,
                    },
                    port: params.port(),
                    key: params.key(),
                    last_announce: params.time(),
                },
            );
        }
    }
    pub(crate) fn evict(&mut self, now: u64, threshold: u64) -> bool {
        self.peers.retain(|_, peer| {
            let is_not_expired = now - peer.last_announce < threshold;
            if !is_not_expired {
                if peer.left == 0 {
                    self.complete -= 1;
                } else {
                    self.incomplete -= 1;
                }
            }
            is_not_expired
        });
        self.is_empty()
    }
}
