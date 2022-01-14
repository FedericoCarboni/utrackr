use std::net::IpAddr;

use crate::core::Event;

#[derive(Debug)]
pub struct AnnounceParams {
    pub(crate) info_hash: [u8; 20],
    pub(crate) peer_id: [u8; 20],
    pub(crate) port: u16,
    pub(crate) remote_ip: IpAddr,
    pub(crate) unsafe_ip: Option<IpAddr>,
    pub(crate) uploaded: i64,
    pub(crate) downloaded: i64,
    pub(crate) left: i64,
    pub(crate) event: Event,
    pub(crate) num_want: i32,
    pub(crate) key: Option<u32>,
    pub(crate) time: u64,
}

impl AnnounceParams {
    /// The info hash specified by the announce request.
    #[inline]
    pub fn info_hash(&self) -> &[u8; 20] {
        &self.info_hash
    }
    /// The self-assigned peer id specified by the announce request.
    #[inline]
    pub fn peer_id(&self) -> &[u8; 20] {
        &self.peer_id
    }
    /// The port specified by the announce request.
    #[inline]
    pub fn port(&self) -> u16 {
        self.port
    }
    #[inline]
    pub fn remote_ip(&self) -> IpAddr {
        self.remote_ip
    }
    /// The self-declared IP address of the peer. May be `None` if not given (or
    /// not supported by the underlying protocol).
    ///
    /// **NEVER assume this to be the correct IP address of the peer**
    #[inline]
    pub fn unsafe_ip(&self) -> Option<IpAddr> {
        self.unsafe_ip
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
    pub fn time(&self) -> u64 {
        self.time
    }
}
