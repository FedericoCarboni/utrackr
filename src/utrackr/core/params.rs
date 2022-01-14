use std::{
    marker::PhantomData,
    net::IpAddr,
    str::{self, FromStr},
    time::{SystemTime, UNIX_EPOCH},
};

use arrayref::array_ref;

use super::{announce::AnnounceParams, Error, Event};

/// An extension to the query parameter parser. It can be used to extract custom
/// parameters from the `?query` part of the announce URL.
///
/// The query parser will call `next` until there are no more key-value pairs to
/// parse, then it will call `try_into` to extract the parsed result.
pub trait ParamsParser<T>: TryInto<T, Error = Error> {
    /// Receives the next key-value pair in the query parameters.
    ///
    /// **NOTE: key and value may contain binary data, do not assume they're
    /// valid UTF-8!**
    fn parse(&mut self, key: &[u8], value: &[u8]) -> Result<(), Error>;
}

/// A no op query parameter parser extension. Used to signal that a parameter
/// parser extension allows chaining.
///
/// The query parameters will be parsed anyway to verify their validity, but
/// they will not be deserialized.
#[derive(Debug, Clone, Copy)]
pub struct EmptyParamsParser;

impl TryInto<()> for EmptyParamsParser {
    type Error = Error;

    #[inline]
    fn try_into(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl ParamsParser<()> for EmptyParamsParser {
    #[inline]
    fn parse(&mut self, _: &[u8], _: &[u8]) -> Result<(), Error> {
        Ok(())
    }
}

#[inline]
fn parse<F: FromStr>(v: &[u8]) -> Result<F, ()> {
    str::from_utf8(v).map_err(|_| ())?.parse().map_err(|_| ())
}

#[derive(Debug)]
pub struct ParseAnnounceParams<T, P>
where
    T: Sync + Send,
    P: ParamsParser<T>,
{
    info_hash: Option<[u8; 20]>,
    peer_id: Option<[u8; 20]>,
    port: u16,
    remote_ip: IpAddr,
    unsafe_ip: Option<IpAddr>,
    uploaded: Option<i64>,
    downloaded: Option<i64>,
    left: Option<i64>,
    event: Option<Event>,
    num_want: Option<i32>,
    key: Option<u32>,
    // support for tracker id should be considered
    // tracker_id: Option<[u8; ]>,
    /// Allow support for a chain of extensions
    extension: P,
    // make the compiler happy
    _marker: PhantomData<T>,
}

impl<T: Sync + Send, P: ParamsParser<T>> ParseAnnounceParams<T, P> {
    #[inline]
    pub fn with_extension(remote_ip: IpAddr, extension: P) -> Self {
        ParseAnnounceParams {
            extension,
            info_hash: None,
            peer_id: None,
            port: 0,
            remote_ip,
            unsafe_ip: None,
            uploaded: None,
            downloaded: None,
            left: None,
            event: None,
            num_want: None,
            key: None,
            // trackerid: Option<[u8; ]>,
            _marker: PhantomData,
        }
    }
}

impl<T: Sync + Send, P: ParamsParser<T>> TryInto<(AnnounceParams, T)>
    for ParseAnnounceParams<T, P>
{
    type Error = Error;

    #[inline]
    fn try_into(self) -> Result<(AnnounceParams, T), Self::Error> {
        if self.port == 0 {
            return Err(Error::InvalidPort);
        }
        match (self.info_hash, self.peer_id) {
            (Some(info_hash), Some(peer_id)) => Ok((
                AnnounceParams {
                    info_hash,
                    peer_id,
                    port: self.port,
                    remote_ip: self.remote_ip,
                    unsafe_ip: self.unsafe_ip,
                    uploaded: self.uploaded.unwrap_or(0),
                    downloaded: self.downloaded.unwrap_or(0),
                    left: self.left.unwrap_or(i64::MAX),
                    event: self.event.unwrap_or(Event::None),
                    num_want: self.num_want.unwrap_or(-1),
                    key: self.key,
                    time: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                },
                self.extension.try_into()?,
            )),
            (None, _) => Err(Error::InvalidInfoHash),
            (_, None) => Err(Error::InvalidPeerId),
        }
    }
}

impl<T: Sync + Send, P: ParamsParser<T>> ParamsParser<(AnnounceParams, T)>
    for ParseAnnounceParams<T, P>
{
    fn parse(&mut self, key: &[u8], value: &[u8]) -> Result<(), Error> {
        match key {
            b"info_hash" => {
                if self.info_hash.is_some() || value.len() != 20 {
                    return Err(Error::InvalidInfoHash);
                }
                self.info_hash = Some(*array_ref!(value, 0, 20));
            }
            b"peer_id" => {
                if self.peer_id.is_some() || value.len() != 20 {
                    return Err(Error::InvalidPeerId);
                }
                self.peer_id = Some(*array_ref!(value, 0, 20));
            }
            b"port" => {
                if self.port != 0 || value.len() > 5 || value.is_empty() {
                    return Err(Error::InvalidPort);
                }
                self.port = parse(value).map_err(|_| Error::InvalidPort)?;
                if self.port == 0 {
                    return Err(Error::InvalidPort);
                }
            }
            b"uploaded" => {
                if self.uploaded.is_some()
                    || value.len() > 19
                    || value.is_empty()
                {
                    return Err(Error::InvalidParams);
                }
                self.uploaded =
                    Some(parse(value).map_err(|_| Error::InvalidParams)?);
            }
            b"downloaded" => {
                if self.downloaded.is_some()
                    || value.len() > 19
                    || value.is_empty()
                {
                    return Err(Error::InvalidParams);
                }
                self.downloaded =
                    Some(parse(value).map_err(|_| Error::InvalidParams)?);
            }
            b"left" => {
                if self.left.is_some() || value.len() > 19 || value.is_empty() {
                    return Err(Error::InvalidParams);
                }
                self.left =
                    Some(parse(value).map_err(|_| Error::InvalidParams)?);
            }
            b"event" => {
                if self.event.is_some() {
                    return Err(Error::InvalidParams);
                }
                self.event = Some(match value {
                    b"started" => Event::Started,
                    b"stopped" => Event::Stopped,
                    b"completed" => Event::Completed,
                    // b"paused" => Event::Paused,
                    _ => Event::None,
                });
            }
            b"ip" => {
                if self.unsafe_ip.is_some() {
                    return Err(Error::InvalidParams);
                }
                self.unsafe_ip =
                    Some(parse(value).map_err(|_| Error::InvalidParams)?);
            }
            b"numwant" => {
                if self.num_want.is_some() {
                    return Err(Error::InvalidParams);
                }
                self.num_want =
                    Some(parse(value).map_err(|_| Error::InvalidParams)?);
            }
            b"key" => {
                if self.key.is_some() {
                    return Err(Error::InvalidParams);
                }
                self.key =
                    Some(parse(value).map_err(|_| Error::InvalidParams)?);
            }
            _ => {
                self.extension.parse(key, value)?;
            }
        }
        Ok(())
    }
}
