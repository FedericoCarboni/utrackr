use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    time::{Duration, Instant},
};

use rand::seq::IteratorRandom;

use crate::core::{Error, MAX_NUM_WANT};

#[derive(Debug, Copy, Clone)]
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
    pub ip_param: Option<IpAddr>,
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
#[derive(Debug, Default)]
pub struct Swarm {
    complete: i32,
    incomplete: i32,
    downloaded: i32,
    peers: BTreeMap<[u8; 20], Peer>,
}

/// Randomly sample exactly `amount` indices from `0..length`, using Floyd's
/// combination algorithm.
///
/// The output values are fully shuffled. (Overhead is under 50%.)
///
/// This implementation uses `O(amount)` memory and `O(amount^2)` time.
fn sample_floyd<R>(rng: &mut impl rand::Rng, amount: u32) -> [u32; MAX_NUM_WANT] {
    let mut indices = [0; MAX_NUM_WANT];
    let mut i = 0;
    for j in 0..amount {
        let t = rng.gen_range(0..=j);
        if indices.contains(&t) {
            indices[i] = j;
        } else {
            indices[i] = t;
        }
    }
    // Reimplement SliceRandom::shuffle with smaller indices
    for i in (1..amount).rev() {
        // invariant: elements with index > i have been locked in place.
        indices.swap(i as usize, rng.gen_range(0..=i) as usize);
    }
    return indices;
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
