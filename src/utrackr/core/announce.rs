use std::net::IpAddr;

use crate::core::Event;

#[derive(Debug)]
pub struct AnnounceParams<T> {
    pub(in crate::core) info_hash: [u8; 20],
    pub(in crate::core) peer_id: [u8; 20],
    pub(in crate::core) port: u16,
    pub(in crate::core) uploaded: i64,
    pub(in crate::core) downloaded: i64,
    pub(in crate::core) left: i64,
    pub(in crate::core) event: Event,
    pub(in crate::core) num_want: i32,
    pub(in crate::core) key: Option<u32>,
    pub(in crate::core) ip: Option<IpAddr>,

    pub(in crate::core) extension: T,
}

impl<T> AnnounceParams<T> {
    /// The info hash specified by the announce request.
    #[inline]
    pub fn info_hash(&self) -> [u8; 20] {
        self.info_hash
    }
    /// The self-assigned peer id specified by the announce request.
    #[inline]
    pub fn peer_id(&self) -> [u8; 20] {
        self.peer_id
    }
    /// The port specified by the announce request.
    #[inline]
    pub fn port(&self) -> u16 {
        self.port
    }
    #[inline]
    pub fn ip(&self) -> Option<IpAddr> {
        self.ip
    }
    #[inline]
    pub fn uploaded(&self) -> i64 {
        self.uploaded
    }
    #[inline]
    pub fn downloaded(&self) -> i64 {
        self.downloaded
    }
    #[inline]
    pub fn left(&self) -> i64 {
        self.left
    }
    #[inline]
    pub fn event(&self) -> Event {
        self.event
    }
    #[inline]
    pub fn num_want(&self) -> i32 {
        self.num_want
    }
    #[inline]
    pub fn key(&self) -> Option<u32> {
        self.key
    }
    #[inline]
    pub fn extension(&self) -> &T {
        &self.extension
    }
}
