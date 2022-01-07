use std::{fmt, error};

/// A tracker error, it indicates a problem with a client's request.
pub enum Error {
    AccessDenied,
    InvalidPort,
    IpAddressChanged,
    TorrentNotFound,
}

impl Error {
    /// Returns a human readable error message for this error,
    /// All error messages only contain characters in the ASCII printable range.
    #[inline]
    pub const fn message(&self) -> &'static str {
        match self {
            Error::AccessDenied => "access denied",
            Error::InvalidPort => "invalid port",
            Error::IpAddressChanged => "ip address changed",
            Error::TorrentNotFound => "torrent not found",
        }
    }
}

impl fmt::Debug for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl error::Error for Error {}
