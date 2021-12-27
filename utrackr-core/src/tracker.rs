use std::{
    collections::HashMap,
    io,
    net::{Ipv4Addr, Ipv6Addr},
    sync::Arc,
    time::Duration,
};

use rand::{seq::IteratorRandom, thread_rng};
use tokio::sync::RwLock;
use mongodb::{
    bson::{doc, spec::BinarySubtype, Binary, Bson, DateTime},
    options::FindOptions,
    Client, Collection,
};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Serialize, Deserialize)]
pub struct Peer {
    #[serde(serialize_with = "self::serialize_bytes_as_binary")]
    #[serde(deserialize_with = "self::deserialize_bytes_as_binary")]
    pub peer_id: [u8; 20],
    #[serde(serialize_with = "self::serialize_bytes_as_binary")]
    #[serde(deserialize_with = "self::deserialize_bytes_as_binary")]
    pub info_hash: [u8; 20],
    #[serde(serialize_with = "self::serialize_duration_as_date")]
    #[serde(deserialize_with = "self::deserialize_duration_as_date")]
    pub last_seen: Duration,
    pub downloaded: u64,
    pub uploaded: u64,
    pub left: u64,
    pub event: i32,
    #[serde(serialize_with = "self::serialize_bytes_as_binary")]
    #[serde(deserialize_with = "self::deserialize_bytes_as_binary_16")]
    pub ip: [u8; 16],
    pub port: u16,
    pub is_ipv4: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Torrent {
    #[serde(serialize_with = "self::serialize_bytes_as_binary")]
    #[serde(deserialize_with = "self::deserialize_bytes_as_binary")]
    pub(crate) info_hash: [u8; 20],
    pub(crate) seeders: i32,
    pub(crate) downloads: i32,
    pub(crate) leechers: i32,
}

fn deserialize_bytes_as_binary<'de, D>(deserializer: D) -> Result<[u8; 20], D::Error>
where
    D: Deserializer<'de>,
{
    match Bson::deserialize(deserializer) {
        Ok(Bson::Binary(binary)) => {
            let mut r = [0u8; 20];
            r.copy_from_slice(&binary.bytes);
            Ok(r)
        }
        Ok(..) => Err(de::Error::invalid_value(
            de::Unexpected::Enum,
            &"Bson::Binary",
        )),
        Err(e) => Err(e),
    }
}

fn serialize_bytes_as_binary<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let binary = Bson::Binary(Binary {
        subtype: BinarySubtype::Generic,
        bytes: bytes.to_vec(),
    });
    binary.serialize(serializer)
}

fn deserialize_bytes_as_binary_16<'de, D>(deserializer: D) -> Result<[u8; 16], D::Error>
where
    D: Deserializer<'de>,
{
    match Bson::deserialize(deserializer) {
        Ok(Bson::Binary(binary)) => {
            let mut r = [0u8; 16];
            r.copy_from_slice(&binary.bytes);
            Ok(r)
        }
        Ok(..) => Err(de::Error::invalid_value(
            de::Unexpected::Enum,
            &"Bson::Binary",
        )),
        Err(e) => Err(e),
    }
}
fn serialize_duration_as_date<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let binary = Bson::DateTime(DateTime::from_millis(duration.as_millis() as _));
    binary.serialize(serializer)
}

fn deserialize_duration_as_date<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    match Bson::deserialize(deserializer) {
        Ok(Bson::DateTime(datetime)) => Ok(Duration::from_millis(datetime.timestamp_millis() as _)),
        Ok(..) => Err(de::Error::invalid_value(
            de::Unexpected::Enum,
            &"Bson::Binary",
        )),
        Err(e) => Err(e),
    }
}

#[derive(Debug, Clone)]
pub struct Tracker {
    // client: Client,
    torrents_coll: Collection<Torrent>,
    peers: Collection<Peer>,
    pub(crate) torrents: Arc<RwLock<HashMap<[u8; 20], Torrent>>>,
    pub(crate) peers_: Arc<RwLock<Vec<Peer>>>,
}

impl Tracker {
    pub async fn new() -> Result<Self, mongodb::error::Error> {
        let client = Client::with_uri_str("mongodb://localhost:27017").await?;
        let db = client.database("test");
        let torrents_coll = db.collection::<Torrent>("utrackr_torrents");
        let peers = db.collection::<Peer>("utrackr_peers");
        Ok(Self {
            // client,
            torrents_coll,
            peers,
            torrents: Default::default(),
            peers_: Default::default()
        })
    }
    pub async fn announce(
        &self,
        peer: &Peer,
        num_want: i32,
        rpkt: &mut [u8],
    ) -> io::Result<()> {
        rpkt[8..12].copy_from_slice(&1800i32.to_be_bytes());
        let torrents = self.torrents.read().await;
        if let Some(torrent) = torrents.get(&peer.info_hash) {
            rpkt[12..16].copy_from_slice(&torrent.leechers.to_be_bytes());
            rpkt[16..20].copy_from_slice(&torrent.seeders.to_be_bytes());
        } else {
            return Ok(());
        }
        let peers = self.peers_.read().await;
        let peers = peers.iter()
            .filter(|npeer| npeer.info_hash == peer.info_hash && npeer.peer_id != peer.peer_id && (!peer.is_ipv4 || npeer.is_ipv4))
            .choose_multiple(&mut thread_rng(), num_want as usize);
        let mut offset = 20;
        if peer.is_ipv4 { // IPv4 ANNOUNCE
            for peer in peers {
                rpkt[offset..offset + 4].copy_from_slice(&Ipv6Addr::from(peer.ip).to_ipv4().unwrap().octets());
                rpkt[offset + 4..offset + 6].copy_from_slice(&peer.port.to_be_bytes());
                offset += 6;
            }
        } else {
            // IPv6 announce
            for peer in peers {
                rpkt[offset..offset + 16].copy_from_slice(&Ipv6Addr::from(peer.ip).octets());
                rpkt[offset + 16..offset + 18].copy_from_slice(&peer.port.to_be_bytes());
                offset += 18;
            }
        }
        Ok(())
    }
    pub async fn add_torrent_or_peer(&self, peer: Peer) {
        let has_peer = self.peers_.read().await.iter().find(|p| p.peer_id == peer.peer_id).is_some();
        let mut torrents = self.torrents.write().await;
        let mut peers = self.peers_.write().await;
        let mut torrent = if let Some(torrent) = torrents.get_mut(&peer.info_hash) {
            torrent
        } else {
            let t = Torrent {
                info_hash: peer.info_hash,
                seeders: 0,
                leechers: 0,
                downloads: 0,
            };
            torrents.insert(t.info_hash, t);
            torrents.get_mut(&peer.info_hash).expect("failed to save torrent")
        };
        if !has_peer && peer.event == 1 || peer.left == 0 {
            torrent.seeders += 1;
        } else if has_peer && peer.event == 3 {
            // if peer.left == 0 {
            //     torrent.seeders -= 1;
            // } else {
            //     torrent.leechers -= 1;
            // }
            peers.iter()
                .position(|npeer| npeer.peer_id == peer.peer_id)
                .map(|i| peers.remove(i));
        } else if !has_peer {
            torrent.leechers += 1;
        }
        if !has_peer {
            peers.push(peer);
        }
    }
    pub async fn scrape(
        &self,
        info_hash: [u8; 20],
    ) -> Result<(i32, i32, i32), mongodb::error::Error> {
        let torrent = self
            .torrents_coll
            .find_one(Some(doc! { "info_hash": Binary{ subtype: BinarySubtype::Generic, bytes: info_hash.to_vec() } }), None)
            .await?;
        if let Some(torrent) = torrent {
            Ok((torrent.seeders, torrent.leechers, torrent.downloads))
        } else {
            let torrent = Torrent {
                info_hash,
                seeders: 0,
                leechers: 0,
                downloads: 0,
            };
            self.torrents_coll.insert_one(&torrent, None).await?;
            Ok((0, 0, 0))
        }
    }
}
