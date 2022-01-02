use std::{collections::HashMap, net::SocketAddr, time::Duration};

use rand::seq::IteratorRandom;

#[derive(Debug)]
pub struct Peer {
    pub(crate) id: [u8; 20],
    pub(crate) key: [u8; 4],
    pub(crate) addr: SocketAddr,
    pub(crate) downloaded: u64,
    pub(crate) left: u64,
    pub(crate) uploaded: u64,
    pub(crate) event: i32, // 0: none; 1: completed; 2: started; 3: stopped
    pub(crate) announced: Duration,
}

#[derive(Debug)]
pub(crate) struct Torrent {
    pub(crate) leechers: i32,
    pub(crate) seeders: i32,
    pub(crate) downloads: i32,
    pub(crate) peers: Vec<Peer>,
}

impl Default for Torrent {
    fn default() -> Self {
        Self {
            leechers: 0,
            seeders: 0,
            downloads: 0,
            peers: Vec::with_capacity(128),
        }
    }
}

#[derive(Debug)]
pub struct Tracker {
    torrents: HashMap<[u8; 20], Torrent>,
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            torrents: Default::default(),
        }
    }
    pub fn read_announce(
        &self,
        peer: &Peer,
        info_hash: [u8; 20],
        amount: usize,
    ) -> (i32, i32, Vec<SocketAddr>) {
        let torrents = &self.torrents;
        if let Some(torrent) = torrents.get(&info_hash) {
            let addrs = torrent
                .peers
                .iter()
                .filter(|p| p.id != peer.id && p.key != peer.key && (peer.addr.is_ipv6() || p.addr.is_ipv4()))
                .map(|p| p.addr)
                .choose_multiple(&mut rand::thread_rng(), amount);
            (torrent.leechers, torrent.seeders, addrs)
        } else {
            (0, 0, Vec::new())
        }
    }
    pub fn write_announce(&mut self, peer: Peer, info_hash: [u8; 20]) {
        let torrent = self.torrents.entry(info_hash).or_default();
        torrent.peers.retain(|p| {
            p.announced - peer.announced < Duration::from_secs(1800)
        });
        let removed = torrent
            .peers
            .iter()
            .position(|p| p.id == peer.id && p.addr == peer.addr)
            .map(|i| torrent.peers.remove(i))
            .is_some();
        if peer.left == 0 && (peer.event == 1 || !removed) {
            torrent.seeders += 1;
        } else if !removed {
            torrent.leechers += 1;
        }
        torrent.peers.push(peer);
    }
    pub fn scrape(&self, info_hash: [u8; 20]) -> (i32, i32, i32) {
        self.torrents
            .get(&info_hash)
            .map(|torrent| (torrent.leechers, torrent.seeders, torrent.downloads))
            .unwrap_or((0, 0, 0))
    }
}
