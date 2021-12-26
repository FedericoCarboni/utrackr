use std::{
    net::{Ipv4Addr, Ipv6Addr},
    time::Duration,
};

use futures::stream::TryStreamExt;
use mongodb::{
    bson::{doc, spec::BinarySubtype, DateTime, Binary, Bson},
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
pub struct Torrent {
    #[serde(serialize_with = "self::serialize_bytes_as_binary")]
    #[serde(deserialize_with = "self::deserialize_bytes_as_binary")]
    pub info_hash: [u8; 20],
    pub seeders: i32,
    pub downloads: i32,
    pub leechers: i32,
    pub peers: i32,
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

}fn serialize_duration_as_date<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
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
        Ok(Bson::DateTime(datetime)) => {
            Ok(Duration::from_millis(datetime.timestamp_millis() as _))
        }
        Ok(..) => Err(de::Error::invalid_value(
            de::Unexpected::Enum,
            &"Bson::Binary",
        )),
        Err(e) => Err(e),
    }
}

#[derive(Clone)]
pub struct Tracker {
    // client: Client,
    torrents: Collection<Torrent>,
    peers: Collection<Peer>,
}

impl Tracker {
    pub async fn new() -> Result<Self, mongodb::error::Error> {
        let client = Client::with_uri_str("mongodb://localhost:27017").await?;
        let db = client.database("test");
        let torrents = db.collection::<Torrent>("utrackr_torrents");
        let peers = db.collection::<Peer>("utrackr_peers");
        Ok(Self {
            // client,
            torrents,
            peers,
        })
    }
    pub async fn announce(
        &self,
        peer: Peer,
        num_want: usize,
        rpkt: &mut [u8],
    ) -> Result<(), mongodb::error::Error> {
        let torrent = self
            .torrents
            .find_one(Some(doc! { "info_hash": Binary{ subtype: BinarySubtype::Generic, bytes: peer.info_hash.to_vec()} }), None)
            .await?;
        if let Some(torrent) = torrent {
            let mut peers = self
                .peers
                .find(
                    Some(doc! {
                        "peer_id": {
                            "$ne": Binary{ subtype: BinarySubtype::Generic, bytes: peer.peer_id.to_vec() },
                        },
                        "info_hash": Binary{ subtype: BinarySubtype::Generic, bytes: peer.info_hash.to_vec() },
                        "is_ipv4": peer.is_ipv4
                    }),
                    FindOptions::builder()
                        .sort(doc! { "left": if peer.left > 0 { -1 } else { 1 } })
                        .limit(Some(num_want as _))
                        .build(),
                )
                .await?;
            if let None = self.peers.find_one(Some(doc! {
                "peer_id": Binary{ subtype: BinarySubtype::Generic, bytes: peer.peer_id.to_vec() },
            }), None).await? {
                self.peers.insert_one(&peer, None).await?;
            }
            // interval
            rpkt[8..12].copy_from_slice(&1800i32.to_be_bytes());
            // leechers
            rpkt[12..16].copy_from_slice(&torrent.leechers.to_be_bytes());
            // seeders
            rpkt[16..20].copy_from_slice(&torrent.seeders.to_be_bytes());
            let mut offset = 20;
            while let Some(found_peer) = peers.try_next().await? {
                if !peer.is_ipv4 {
                    rpkt[offset..offset + 16].copy_from_slice(&found_peer.ip);
                    rpkt[offset + 16..offset + 18].copy_from_slice(&found_peer.port.to_be_bytes());
                    offset += 18;
                } else {
                    rpkt[offset..offset + 4].copy_from_slice(
                        &Ipv6Addr::from(found_peer.ip)
                            .to_ipv4()
                            .unwrap_or(Ipv4Addr::new(0, 0, 0, 0))
                            .octets(),
                    );
                    rpkt[offset + 4..offset + 6].copy_from_slice(&found_peer.port.to_be_bytes());
                    offset += 6;
                }
            }
        } else {
            let torrent = Torrent {
                info_hash: peer.info_hash,
                seeders: 0,
                leechers: 0,
                downloads: 0,
                peers: 0,
            };
            self.torrents.insert_one(&torrent, None).await?;
        }
        Ok(())
    }
    pub async fn scrape(
        &self,
        info_hash: [u8; 20],
    ) -> Result<(i32, i32, i32), mongodb::error::Error> {
        let torrent = self
            .torrents
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
                peers: 0,
            };
            self.torrents.insert_one(&torrent, None).await?;
            Ok((0, 0, 0))
        }
    }
}
