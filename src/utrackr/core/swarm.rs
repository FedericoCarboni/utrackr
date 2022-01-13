use std::{collections::BTreeMap, net::IpAddr};

use rand::seq::IteratorRandom;

use crate::core::announce::AnnounceParams;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Event {
    None,
    Completed,
    Started,
    Stopped,
    // BEP 21
    // Paused,
}

#[derive(Debug)]
pub struct Peer {
    pub downloaded: i64,
    pub uploaded: i64,
    pub left: i64,
    // interop support for IPv6/IPv4 is missing
    pub ip: IpAddr,
    pub port: u16,
    pub key: Option<u32>,
    pub last_announce: u64,
}

impl Peer {
    #[inline]
    pub fn is_seeder(&self) -> bool {
        self.left == 0
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
            .filter(|(id, peer)| {
                // don't announce peers to themselves
                *id != peer_id
                    // don't announce seeders to other seeders
                    && (peer.is_seeder() || !seeding)
                    // don't announce IPv6 peers to IPv4 peers, but allow IPv4
                    // addresses in IPv6 announces.
                    && (ip.is_ipv6() || peer.ip.is_ipv4())
            })
            .map(|(_, peer)| (peer.ip, peer.port))
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
            peer.ip = ip;
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
                    ip,
                    port: params.port(),
                    key: params.key(),
                    last_announce: params.time(),
                },
            );
        }
    }
    pub(crate) fn evict(&mut self, now: u64, threshold: u64) {
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
    }
}
