use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use futures::future::join_all;
use tokio::sync::RwLock;

use crate::config::TrackerConfig;
use crate::swarm::{Announce, AnnounceError, Event, Swarm};

struct TrackerInner {
    swarms: RwLock<HashMap<[u8; 20], RwLock<Swarm>>>,
    config: TrackerConfig,
}

impl TrackerInner {
    pub fn new(config: TrackerConfig) -> Self {
        Self {
            swarms: RwLock::new(HashMap::new()),
            config,
        }
    }
    pub async fn announce(
        &self,
        announce: Announce,
    ) -> Result<Option<(i32, i32, Vec<([u8; 20], SocketAddr)>)>, AnnounceError> {
        let swarms = self.swarms.read().await;
        if let Some(swarm) = swarms.get(&announce.info_hash) {
            let swarm_read = swarm.read().await;
            swarm_read.validate(&announce)?;
            let result = if matches!(announce.event, Event::Stopped | Event::Paused) {
                Ok(None)
            } else {
                Ok(Some((
                    swarm_read.complete(),
                    swarm_read.incomplete(),
                    swarm_read.select(&announce),
                )))
            };
            drop(swarm_read);
            swarm.write().await.announce(&announce);
            result
        } else if !self.config.enable_unknown_torrents {
            Err(AnnounceError::UnknownTorrent)
        } else {
            drop(swarms);
            let mut swarms = self.swarms.write().await;
            let mut swarm = Swarm::default();
            swarm.announce(&announce);
            swarms.insert(announce.info_hash, RwLock::new(swarm));
            // Acquire a write lock to the swarms map before we do anything else
            // this will make sure that we don't have any race conditions
            Ok(None)
        }
    }
    pub async fn scrape(&self, info_hashes: &[[u8; 20]]) -> Vec<(i32, i32, i32)> {
        let swarms = self.swarms.read().await;
        join_all(info_hashes.iter().map(|info_hash| async {
            if let Some(swarm) = swarms.get(info_hash) {
                let swarm = swarm.read().await;
                (swarm.complete(), swarm.incomplete(), swarm.downloaded())
            } else {
                (0, 0, 0)
            }
        }))
        .await
    }
    pub async fn evict(&self) {
        let now = Instant::now();
        let threshold = Duration::from_secs(self.config.max_interval);
        let swarms = self.swarms.read().await;
        join_all(
            swarms
                .iter()
                .map(|(_, swarm)| async { swarm.write().await.evict(now, threshold) }),
        )
        .await;
    }
}

#[derive(Clone)]
pub struct Tracker {
    inner: Arc<TrackerInner>,
}

impl Tracker {
    pub fn new(config: TrackerConfig) -> Self {
        Self {
            inner: Arc::new(TrackerInner::new(config)),
        }
    }
    pub fn config(&self) -> &TrackerConfig {
        &self.inner.config
    }
    pub async fn scrape(&self, info_hashes: &[[u8; 20]]) -> Vec<(i32, i32, i32)> {
        self.inner.scrape(info_hashes).await
    }
    pub async fn announce(
        &self,
        mut announce: Announce,
    ) -> Result<Option<(i32, i32, Vec<([u8; 20], SocketAddr)>)>, AnnounceError> {
        if announce.num_want < 0 {
            announce.num_want = self.inner.config.default_num_want;
        } else if announce.num_want > self.inner.config.max_num_want {
            announce.num_want = self.inner.config.max_num_want;
        }
        self.inner.announce(announce).await
    }
    pub fn start_autosave(&self) {
        let tracker = self.inner.clone();
        let mut int = tokio::time::interval(Duration::from_secs(20));
        tokio::spawn(async move {
            loop {
                int.tick().await;
                tracker.evict().await;
            }
        });
    }
}

impl Default for Tracker {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

#[cfg(test)]
mod test {
    use std::net::SocketAddr;

    use super::*;

    #[derive(Clone)]
    struct MockAnnounce {
        announce: Announce,
    }

    impl MockAnnounce {
        pub fn new() -> Self {
            Self {
                announce: Announce {
                    info_hash: [0u8; 20],
                    peer_id: [0u8; 20],
                    downloaded: 0,
                    uploaded: 0,
                    left: i64::MAX,
                    addr: ([150, 150, 150, 150], 6881).into(),
                    event: Event::Started,
                    key: None,
                    num_want: 32,
                    instant: Instant::now(),
                },
            }
        }
        pub fn mock(self) -> Announce {
            self.announce
        }
        pub fn with_info_hash(mut self, info_hash: [u8; 20]) -> Self {
            self.announce.info_hash = info_hash;
            self
        }
        pub fn with_peer_id(mut self, peer_id: [u8; 20]) -> Self {
            self.announce.peer_id = peer_id;
            self
        }
        pub fn with_downloaded(mut self, downloaded: i64) -> Self {
            self.announce.downloaded = downloaded;
            self
        }
        pub fn with_uploaded(mut self, uploaded: i64) -> Self {
            self.announce.uploaded = uploaded;
            self
        }
        pub fn with_left(mut self, left: i64) -> Self {
            self.announce.left = left;
            self
        }
        pub fn with_addr(mut self, addr: SocketAddr) -> Self {
            self.announce.addr = addr;
            self
        }
        pub fn with_event(mut self, event: Event) -> Self {
            self.announce.event = event;
            self
        }
        pub fn with_key(mut self, key: u32) -> Self {
            self.announce.key = Some(key);
            self
        }
        pub fn with_num_want(mut self, num_want: i32) -> Self {
            self.announce.num_want = num_want;
            self
        }
        pub fn with_instant(mut self, instant: Instant) -> Self {
            self.announce.instant = instant;
            self
        }
    }

    #[tokio::test]
    async fn test_simple_announce() {
        let tracker = Tracker::default();
        tracker
            .announce(MockAnnounce::new().with_peer_id([1; 20]).mock())
            .await
            .unwrap();
        tracker
            .announce(
                MockAnnounce::new()
                    .with_peer_id([2; 20])
                    .with_left(0)
                    .mock(),
            )
            .await
            .unwrap();
        tracker
            .announce(MockAnnounce::new().with_peer_id([3; 20]).mock())
            .await
            .unwrap();
        let peers = tracker
            .announce(MockAnnounce::new().with_left(0).mock())
            .await
            .unwrap();
        assert!(peers.is_some());
        let (_, _, peers) = peers.unwrap();
        assert_eq!(peers.len(), 2);
        assert_eq!(
            peers.iter().position(|(peer_id, _)| peer_id == &[2; 20]),
            None
        );
    }

    #[tokio::test]
    async fn test_announce_deny_ip_change_without_key() {
        let tracker = Tracker::default();
        tracker
            .announce(MockAnnounce::new().with_peer_id([1u8; 20]).mock())
            .await
            .unwrap();
        tracker
            .announce(
                MockAnnounce::new()
                    .with_peer_id([1u8; 20])
                    .with_addr(([150, 150, 150, 151], 6881).into())
                    .mock(),
            )
            .await
            .unwrap_err();
    }

    #[tokio::test]
    async fn test_announce_deny_ip_change_with_wrong_key() {
        let tracker = Tracker::default();
        tracker
            .announce(
                MockAnnounce::new()
                    .with_peer_id([1u8; 20])
                    .with_key(12345)
                    .mock(),
            )
            .await
            .unwrap();
        tracker
            .announce(
                MockAnnounce::new()
                    .with_peer_id([1u8; 20])
                    .with_addr(([150, 150, 150, 151], 6881).into())
                    .with_key(0)
                    .mock(),
            )
            .await
            .unwrap_err();
    }

    #[tokio::test]
    async fn test_announce_accept_ip_change_with_key() {
        let tracker = Tracker::default();
        tracker
            .announce(
                MockAnnounce::new()
                    .with_peer_id([1u8; 20])
                    .with_key(12345)
                    .mock(),
            )
            .await
            .unwrap();
        tracker
            .announce(
                MockAnnounce::new()
                    .with_peer_id([1u8; 20])
                    .with_addr(([150, 150, 150, 151], 6881).into())
                    .with_key(12345)
                    .mock(),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_scrape() {
        let tracker = Tracker::default();
        tracker
            .announce(
                MockAnnounce::new()
                    .with_peer_id([1u8; 20])
                    .with_event(Event::Completed)
                    .with_left(0)
                    .mock(),
            )
            .await
            .unwrap();
        tracker
            .announce(MockAnnounce::new().with_peer_id([2u8; 20]).mock())
            .await
            .unwrap();
        let results = tracker.scrape(&[[0; 20]]).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], Some((1, 1, 1)));
    }

    #[tokio::test]
    async fn test_eviction() {
        let mut config = TrackerConfig::default();
        config.max_interval = 0;
        let tracker = Tracker::new(config);
        let instant = Instant::now();
        tracker
            .announce(MockAnnounce::new().with_instant(instant).mock())
            .await
            .unwrap();
        tracker.inner.evict().await;
        assert_eq!(tracker.scrape(&[[0; 20]]).await, vec![Some((0, 0, 0))]);
    }
}
