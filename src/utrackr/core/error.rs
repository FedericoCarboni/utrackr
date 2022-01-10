use std::{fmt, error};

/// A tracker error, it indicates a problem with a client's request.
pub enum Error {
    AccessDenied,
    InvalidInfoHash,
    InvalidIpAddress,
    InvalidPeerId,
    InvalidPort,
    InvalidParams,
    Internal,
    IpAddressChanged,
    TorrentNotFound,
    Custom(&'static str)
}

impl Error {
    /// Returns a human readable error message for this error,
    /// All error messages only contain characters in the ASCII printable range.
    #[inline]
    pub const fn message(&self) -> &'static str {
        match self {
            Error::AccessDenied => "access denied",
            Error::InvalidInfoHash => "invalid info hash",
            Error::InvalidIpAddress => "invalid IP address",
            Error::InvalidParams => "invalid parameters",
            Error::InvalidPeerId => "invalid peer id",
            Error::InvalidPort => "invalid port",
            Error::Internal => "internal server error",
            Error::IpAddressChanged => "IP address changed",
            Error::TorrentNotFound => "torrent not found",
            Error::Custom(message) => message,
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
