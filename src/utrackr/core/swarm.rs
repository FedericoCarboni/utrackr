use std::{
    collections::BTreeMap,
    net::{IpAddr},
    time::{Duration, Instant},
};

use rand::seq::IteratorRandom;

use crate::core::{Error, announce::AnnounceParams, MAX_NUM_WANT};

#[derive(Debug, Copy, Clone)]
pub enum Event {
    None,
    Completed,
    Started,
    Stopped,
    // Paused,
}

#[derive(Debug)]
pub struct Peer {
    pub downloaded: i64,
    pub uploaded: i64,
    pub left: i64,
    pub port: u16,
    pub ip: IpAddr,
    pub key: Option<u32>,
    pub announced: u64,
}

impl Peer {
    #[inline]
    pub fn is_seeder(&self) -> bool {
        self.left == 0
    }
    #[inline]
    pub fn is_leecher(&self) -> bool {
        !self.is_seeder()
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
    pub fn select(&self, peer_id: &[u8; 20], ip: &IpAddr, seeding: bool, amount: usize) -> Vec<(&[u8; 20], &IpAddr, u16)> {
        self.peers
            .iter()
            .filter(|(id, peer)| {
                // don't announce peers to themselves
                *id != peer_id
                    // don't announce seeders to other seeders
                    && (peer.left != 0 || !seeding)
                    // don't announce IPv6 peers to IPv4 peers, but allow IPv4
                    // addresses in IPv6 announces.
                    && (ip.is_ipv6() || peer.ip.is_ipv4())
            })
            .map(|(id, peer)| (id, &peer.ip, peer.port))
            .choose_multiple(&mut rand::thread_rng(), amount)
    }
    pub fn announce<T>(&mut self, params: &AnnounceParams<T>, ip: IpAddr) {
        match params.event() {
            Event::Completed => {
                self.downloaded += 1;
            }
            Event::Stopped => {
                if let Some(peer) = self.peers.remove(params.peer_id()) {
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
        if let Some(peer) = self.peers.get_mut(params.peer_id()) {
            peer.downloaded = params.downloaded();
            peer.uploaded = params.uploaded();
            peer.left = params.left();
            peer.ip = ip;
            peer.port = params.port();
            peer.key = params.key();
            // peer.announced = Instant::now();
        } else {
            if params.left == 0 {
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
                    announced: 0,
                },
            );
        }
    }
    pub(crate) fn evict(&mut self, now: u64, threshold: u64) {
        self.peers.retain(|_, peer| {
            let is_not_expired = now - peer.announced < threshold;
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
