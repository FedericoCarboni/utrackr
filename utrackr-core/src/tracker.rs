use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug)]
pub struct Peer {
    peer_id: [u8; 20],
    downloaded: u64,
    uploaded: u64,
    left: u64,
    last_seen: Duration,
    flags: u16,
}

#[derive(Debug)]
pub struct PeerList {
    seeders: i64,
    leechers: i64,
    downloads: i64,
    peers: Vec<Peer>,
}

pub struct Tracker {
    torrents: HashMap<[u8; 20], PeerList>,
}

impl Tracker {
    pub fn contains(&self, info_hash: &[u8]) -> bool {
        self.torrents.contains_key(info_hash)
    }
    pub fn select_peers(&self, info_hash: &[u8], num_want: usize) -> &[Peer] {
        let peer_list = if let Some(peer_list) = self.torrents.get(info_hash) {
            peer_list
        } else {
            panic!("");
        };
        let num_want = if peer_list.peers.len() >= num_want {
            num_want
        } else {
            peer_list.peers.len()
        };
        &peer_list.peers[..num_want]
    }
    pub fn insert(&mut self, info_hash: &[u8], peer: Peer) {
        let peer_list = if let Some(peer_list) = self.torrents.get_mut(info_hash) {
            peer_list
        } else {
            let mut owned_info_hash = [0u8; 20];
            owned_info_hash.copy_from_slice(info_hash);
            self.torrents.insert(owned_info_hash, PeerList {
                seeders: 0,
                leechers: 0,
                downloads: 0,
                peers: vec![],
            });
            self.torrents.get_mut(info_hash)
                .expect("failed to add torrent")
        };
    }
}
