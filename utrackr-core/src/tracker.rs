use std::{
    collections::HashMap,
    io,
    net::SocketAddr
};

use bb8_redis::{
    bb8,
    redis::{cmd, Value, RedisResult, FromRedisValue, AsyncCommands},
    RedisConnectionManager
};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Peer {
    pub downloaded: u64,
    pub uploaded: u64,
    pub left: u64,
    pub event: i32,
    pub addr: SocketAddr,
}

impl FromRedisValue for Peer {
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        match v {
            Value::Data(data) => Ok(bincode::deserialize(data).unwrap()),
            _ => unreachable!(),
        }
    }
}

#[derive(Clone)]
pub struct Tracker {
    pool: bb8::Pool<RedisConnectionManager>,
}

impl Tracker {
    pub async fn new() -> Self {
        let manager = RedisConnectionManager::new("redis://localhost").unwrap();
        let pool = bb8::Pool::builder().build(manager).await.unwrap();
        Self {
            pool
        }
    }
    pub async fn add_seeder(&mut self, info_hash: &[u8]) {
        let mut key = [0u8; "utrackr:".len() + 20];
        key[.."utrackr:".len()].copy_from_slice(b"utrackr:");
        key["utrackr:".len()..].copy_from_slice(info_hash);
        let _: i64 = self.pool.get().await.unwrap().hincr(&key, "seeders", 1).await.unwrap();
    }
    pub async fn add_leecher(&mut self, info_hash: &[u8]) {
        let mut key = [0u8; "utrackr:".len() + 20];
        key[.."utrackr:".len()].copy_from_slice(b"utrackr:");
        key["utrackr:".len()..].copy_from_slice(info_hash);
        let _: i64 = self.pool.get().await.unwrap().hincr(&key, "leechers", 1).await.unwrap();
    }
    pub async fn add_downloads(&mut self, info_hash: &[u8]) {
        let mut key = [0u8; "utrackr:".len() + 20];
        key[.."utrackr:".len()].copy_from_slice(b"utrackr:");
        key["utrackr:".len()..].copy_from_slice(info_hash);
        let _: i64 = self.pool.get().await.unwrap().hincr(&key, "completed", 1).await.unwrap();
    }
    pub async fn scrape(&mut self, info_hash: &[u8]) -> (u32, u32, u32) {
        let mut key = [0u8; "utrackr:".len() + 20];
        key[.."utrackr:".len()].copy_from_slice(b"utrackr:");
        key["utrackr:".len()..].copy_from_slice(info_hash);
        let (seeders, leechers, completed): (Option<u32>, Option<u32>, Option<u32>) = self.pool.get().await.unwrap().hget(&key, &["seeders", "leechers", "completed"]).await.unwrap();
        (seeders.unwrap_or(0), leechers.unwrap_or(0), completed.unwrap_or(0))
    }
    pub async fn insert(&mut self, info_hash: &[u8], peer_id: &[u8], peer: Peer) {
        let mut key = [0u8; "utrackr:".len() + 20 + ":peers".len()];
        key[.."utrackr:".len()].copy_from_slice(b"utrackr:");
        key["utrackr:".len().."utrackr:".len() + 20].copy_from_slice(info_hash);
        key["utrackr:".len() + 20..].copy_from_slice(b":peers");
        let _: u8 = self.pool.get().await.unwrap().hset(&key as &[u8], &peer_id, bincode::serialize(&peer).unwrap()).await.unwrap();
    }
    pub async fn select_peers(&mut self, info_hash: &[u8], num_want: usize) -> io::Result<HashMap<Vec<u8>, Peer>> {
        let mut key = [0u8; "utrackr:".len() + 20 + ":peers".len()];
        key[.."utrackr:".len()].copy_from_slice(b"utrackr:");
        key["utrackr:".len().."utrackr:".len() + 20].copy_from_slice(info_hash);
        key["utrackr:".len() + 20..].copy_from_slice(b":peers");
        let s: HashMap<Vec<u8>, Peer> = cmd("HRANDFIELD")
            .arg(&key as &[u8])
            .arg(num_want.to_string())
            .arg("WITHVALUES")
            .query_async(&mut *self.pool.get().await.unwrap()).await.unwrap();
        Ok(s)
    }
}
