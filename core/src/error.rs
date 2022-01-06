
#[derive(Debug)]
pub enum Error {
    TorrentNotFound,
    IpAddrChanged,
    InvalidPort,
    AccessDenied,
}

impl Error {
    /// Returns a human readable error message for this error.
    #[inline]
    pub const fn message(&self) -> &'static str {
        match self {
            Error::TorrentNotFound => "torrent not found",
            Error::IpAddrChanged => "ip address changed",
            Error::InvalidPort => "invalid port",
            Error::AccessDenied => "access denied",
        }
    }
    #[inline]
    pub const fn code(&self) -> u16 {
        match self {
            Error::TorrentNotFound => 404,
            Error::IpAddrChanged => 403,
            Error::InvalidPort => 400,
            Error::AccessDenied => 401,
        }
    }
}
