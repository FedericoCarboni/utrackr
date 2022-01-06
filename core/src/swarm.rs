use std::{
    collections::HashMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

use rand::seq::IteratorRandom;

#[derive(Debug)]
pub enum AnnounceError {
    UnknownTorrent,
    InvalidKey,
}

#[derive(Debug, Clone)]
pub enum Event {
    None,
    Completed,
    Started,
    Stopped,
    Paused,
}

#[derive(Debug, Clone)]
pub struct Announce {
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
    pub downloaded: i64,
    pub uploaded: i64,
    pub left: i64,
    #[cfg(feature = "announce-corrupt-redudant")]
    pub corrupt: i64,
    #[cfg(feature = "announce-corrupt-redudant")]
    pub redudant: i64,
    pub event: Event,
    pub addr: SocketAddr,
    pub key: Option<u32>,
    pub num_want: i32,
    pub instant: Instant,
}

#[derive(Debug)]
pub struct Peer {
    pub downloaded: i64,
    pub uploaded: i64,
    pub left: i64,
    pub addr: SocketAddr,
    pub key: Option<u32>,
    pub announce: Instant,
}

/// In-Memory store of a peer swarm
#[derive(Debug)]
pub struct Swarm {
    complete: i32,
    incomplete: i32,
    downloaded: i32,
    peers: HashMap<[u8; 20], Peer>,
}

impl Default for Swarm {
    fn default() -> Self {
        Self {
            complete: 0,
            incomplete: 0,
            downloaded: 0,
            peers: HashMap::new(),
        }
    }
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
    pub fn validate(&self, announce: &Announce) -> Result<(), AnnounceError> {
        if let Some(peer) = self.peers.get(&announce.peer_id) {
            // BitTorrent clients should use the key parameter to prove their
            // identity should their ip address
            if peer.addr.ip() != announce.addr.ip()
                && (peer.key != announce.key || peer.key.is_none())
            {
                return Err(AnnounceError::InvalidKey);
            }
        }
        Ok(())
    }
    pub fn select(&self, announce: &Announce) -> Vec<([u8; 20], SocketAddr)> {
        self.peers
            .iter()
            // don't announce peers to themselves or announce seeders to other seeders
            .filter(|(&id, peer)| {
                id != announce.peer_id
                    && (peer.left != 0 || announce.left != 0)
                    && (announce.addr.is_ipv6() || peer.addr.is_ipv4())
            })
            .map(|(&id, peer)| (id, peer.addr))
            .choose_multiple(&mut rand::thread_rng(), announce.num_want as usize)
    }
    pub fn announce(&mut self, announce: &Announce) {
        match announce.event {
            Event::Completed => {
                self.downloaded += 1;
            }
            Event::Stopped | Event::Paused => {
                if let Some(peer) = self.peers.remove(&announce.peer_id) {
                    if peer.left == 0 {
                        self.complete -= 1;
                    } else {
                        self.incomplete -= 1;
                    }
                }
                return;
            }
            _ => {}
        }
        if let Some(peer) = self.peers.get_mut(&announce.peer_id) {
            peer.downloaded = announce.downloaded;
            peer.uploaded = announce.uploaded;
            peer.left = announce.left;
            peer.addr = announce.addr;
            peer.key = announce.key;
            peer.announce = announce.instant;
        } else {
            if announce.left == 0 {
                self.complete += 1;
            } else {
                self.incomplete += 1;
            }
            self.peers.insert(
                announce.peer_id,
                Peer {
                    downloaded: announce.downloaded,
                    uploaded: announce.uploaded,
                    left: announce.left,
                    addr: announce.addr,
                    key: announce.key,
                    announce: announce.instant,
                },
            );
        }
    }
    pub(crate) fn evict(&mut self, now: Instant, threshold: Duration) {
        self.peers.retain(|_, peer| {
            let is_not_expired = now - peer.announce < threshold;
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
