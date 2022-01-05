use std::{
    net::{IpAddr, SocketAddr},
    str,
    time::Instant,
};

use arrayref::array_ref;

use crate::urlencoded;
use utrackr_core::{AnnounceRequest, Event};

#[derive(Debug)]
pub(crate) enum ParseQueryError {
    MissingInfoHash,
    InvalidInfoHash,
    MissingPeerId,
    InvalidPeerId,
    InvalidPort,
    MissingPort,
    InvalidNumWant,
    ParseError,
}

impl warp::reject::Reject for ParseQueryError {}

pub(crate) fn parse_announce_query(
    query: &[u8],
    ip: IpAddr,
) -> Result<AnnounceRequest, ParseQueryError> {
    let mut info_hash: Option<[u8; 20]> = None;
    let mut peer_id: Option<[u8; 20]> = None;
    let mut port_seen = false;
    let mut port = 0u16;
    let mut uploaded: Option<u64> = None;
    let mut downloaded: Option<u64> = None;
    let mut left: Option<u64> = None;
    let mut event_seen = false;
    let mut event = Event::None;
    let mut compact: Option<bool> = None;
    let mut num_want: Option<i32> = None;
    let mut key: Option<u32> = None;
    for (name, value) in urlencoded::parse(query) {
        match &*name {
            b"info_hash" => {
                if value.len() != 20 || info_hash.is_some() {
                    return Err(ParseQueryError::InvalidInfoHash);
                }
                info_hash = Some(array_ref![&value, 0, 20].clone());
            }
            b"peer_id" => {
                if value.len() != 20 || peer_id.is_some() {
                    return Err(ParseQueryError::InvalidPeerId);
                }
                peer_id = Some(array_ref![&value, 0, 20].clone());
            }
            b"port" => {
                if port_seen {
                    return Err(ParseQueryError::InvalidPort);
                }
                port_seen = true;
                port = str::from_utf8(&value)
                    .map_err(|_| ParseQueryError::InvalidPort)?
                    .parse()
                    .map_err(|_| ParseQueryError::InvalidPort)?;
            }
            b"uploaded" => {
                if uploaded.is_some() {
                    return Err(ParseQueryError::ParseError);
                }
                uploaded = Some(
                    str::from_utf8(&value)
                        .map_err(|_| ParseQueryError::ParseError)?
                        .parse()
                        .map_err(|_| ParseQueryError::ParseError)?,
                );
            }
            b"downloaded" => {
                if downloaded.is_some() {
                    return Err(ParseQueryError::ParseError);
                }
                downloaded = Some(
                    str::from_utf8(&value)
                        .map_err(|_| ParseQueryError::ParseError)?
                        .parse()
                        .map_err(|_| ParseQueryError::ParseError)?,
                );
            }
            b"left" => {
                if left.is_some() {
                    return Err(ParseQueryError::ParseError);
                }
                left = Some(
                    str::from_utf8(&value)
                        .map_err(|_| ParseQueryError::ParseError)?
                        .parse()
                        .map_err(|_| ParseQueryError::ParseError)?,
                );
            }
            b"event" => {
                if event_seen {
                    return Err(ParseQueryError::ParseError);
                }
                event_seen = true;
                event = match &*value {
                    b"started" => Event::Started,
                    b"stopped" => Event::Stopped,
                    b"completed" => Event::Completed,
                    _ => Event::None,
                };
            }
            b"numwant" => {
                if num_want.is_some() {
                    return Err(ParseQueryError::InvalidNumWant);
                }
                num_want = Some(
                    str::from_utf8(&value)
                        .map_err(|_| ParseQueryError::ParseError)?
                        .parse()
                        .map_err(|_| ParseQueryError::ParseError)?,
                );
            }
            b"compact" => {
                if compact.is_some() {
                    return Err(ParseQueryError::ParseError);
                }
                compact = Some(
                    str::from_utf8(&value)
                        .map_err(|_| ParseQueryError::ParseError)?
                        .parse::<i8>()
                        .map_err(|_| ParseQueryError::ParseError)?
                        == 0,
                );
            }
            b"key" => {
                if key.is_some() {
                    return Err(ParseQueryError::ParseError);
                }
                key = Some(
                    str::from_utf8(&value)
                        .map_err(|_| ParseQueryError::ParseError)?
                        .parse()
                        .map_err(|_| ParseQueryError::ParseError)?
                );
            }
            _ => {}
        }
    }
    if port == 0 {
        return Err(ParseQueryError::InvalidPort);
    }
    match (info_hash, peer_id) {
        (Some(info_hash), Some(peer_id)) => {
            Ok(AnnounceRequest {
                info_hash,
                peer_id,
                addr: SocketAddr::new(ip, port),
                key,
                downloaded: downloaded.unwrap_or(0),
                uploaded: uploaded.unwrap_or(0),
                left: left.unwrap_or(u64::MAX),
                event,
                num_want: num_want.unwrap_or(-1),
                timestamp: Instant::now(),
            })
        }
        (None, _) => Err(ParseQueryError::MissingInfoHash),
        (_, None) => Err(ParseQueryError::MissingPeerId),
    }
}
