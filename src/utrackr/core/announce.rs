use std::net::IpAddr;

use crate::core::Event;

#[derive(Debug)]
pub struct AnnounceParams {
    info_hash: [u8; 20],
    peer_id: [u8; 20],
    port: u16,
    remote_ip: IpAddr,
    unsafe_ip: Option<IpAddr>,
    uploaded: i64,
    downloaded: i64,
    left: i64,
    event: Event,
    num_want: i32,
    key: Option<u32>,
    time: u64,
}

impl AnnounceParams {
    #[inline]
    pub(crate) fn new(
        info_hash: [u8; 20],
        peer_id: [u8; 20],
        port: u16,
        remote_ip: IpAddr,
        unsafe_ip: Option<IpAddr>,
        uploaded: i64,
        downloaded: i64,
        left: i64,
        event: Event,
        num_want: i32,
        key: Option<u32>,
        time: u64,
    ) -> Self {
        Self {
            info_hash,
            peer_id,
            port,
            remote_ip,
            unsafe_ip,
            uploaded,
            downloaded,
            left,
            event,
            num_want,
            key,
            time,
        }
    }
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
