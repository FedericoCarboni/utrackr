use std::{collections::HashMap, net::SocketAddr, time::Instant};

use crate::tracker::{AnnounceError, AnnounceRequest};

#[derive(Debug)]
pub enum Event {
    None,
    Started,
    Stopped,
    Completed,
}

#[derive(Debug)]
pub struct Peer {
    pub(crate) downloaded: u64,
    pub(crate) uploaded: u64,
    pub(crate) left: u64,
    pub(crate) addr: SocketAddr,
    pub(crate) key: Option<u32>,
    pub(crate) announce: Instant,
}

impl Peer {}

#[derive(Debug)]
pub struct Swarm {
    /// Number of seeds in the swarm
    pub(crate) complete: u64,
    /// Number of leeches in the swarm
    pub(crate) incomplete: u64,
    /// Number of times the torrent has been downloaded to completion
    pub(crate) downloaded: u64,
    /// List of peers currently connected
    pub(crate) peers: HashMap<[u8; 20], Peer>,
}

impl Swarm {
    #[must_use]
    #[inline]
    pub fn complete(&self) -> u64 {
        self.complete
    }
    #[must_use]
    #[inline]
    pub fn incomplete(&self) -> u64 {
        self.incomplete
    }
    #[must_use]
    #[inline]
    pub fn downloaded(&self) -> u64 {
        self.downloaded
    }
    #[must_use]
    #[inline]
    pub fn peers(&self) -> &HashMap<[u8; 20], Peer> {
        &self.peers
    }
    #[must_use]
    #[inline]
    pub fn peers_mut(&mut self) -> &mut HashMap<[u8; 20], Peer> {
        &mut self.peers
    }
    fn stop(&mut self, peer_id: &[u8; 20]) {
        if let Some(peer) = self.peers.remove(peer_id) {
            if peer.left == 0 {
                self.complete -= 1;
            } else {
                self.incomplete -= 1;
            }
        }
    }
    pub fn announce(&mut self, req: &AnnounceRequest) -> Result<(), AnnounceError> {
        match req.event {
            Event::None | Event::Started | Event::Completed => {
                let mut has_completed = matches!(req.event, Event::Completed) && req.left == 0;
                let mut had_peer = false;
                let peer = self
                    .peers
                    .entry(req.peer_id)
                    .and_modify(|peer| {
                        if peer.key != req.key {
                            return;
                        }
                        if peer.left > 0 && req.left == 0 {
                            has_completed = true;
                        }
                        had_peer = true;
                        peer.downloaded = req.downloaded;
                        peer.uploaded = req.uploaded;
                        peer.left = req.left;
                        peer.announce = req.timestamp;
                    })
                    .or_insert(Peer {
                        downloaded: req.downloaded,
                        uploaded: req.uploaded,
                        left: req.left,
                        addr: req.addr,
                        key: req.key,
                        announce: req.timestamp,
                    });
                if peer.key != req.key {
                    return Err(AnnounceError::InvalidKey);
                }
                if has_completed {
                    self.complete += 1;
                    self.downloaded += 1;
                } else if !had_peer {
                    self.incomplete += 1;
                }
            }
            Event::Stopped => {
                self.stop(&req.peer_id);
            }
        }
        Ok(())
    }
}

impl Default for Swarm {
    fn default() -> Self {
        Self {
            complete: 0,
            incomplete: 0,
            downloaded: 0,
            peers: Default::default(),
        }
    }
}
