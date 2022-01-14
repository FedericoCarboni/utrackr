use std::{error, fmt};

/// A tracker error, it indicates a problem with a client's request.
pub enum Error {
    /// The client is not allowed to access the tracker.
    AccessDenied,
    /// The client sent an announce request to an invalid URL.
    InvalidAnnounceUrl,
    /// The client sent an info hash not 20 bytes in length.
    InvalidInfoHash,
    /// The client sent an `ip` param, but it was malformed or invalid.
    InvalidIpAddress,
    /// The client sent a peer ID not 20 bytes long.
    InvalidPeerId,
    /// The client sent 0, or a system port as `port`.
    InvalidPort,
    /// The client sent invalid or malformed parameters.
    InvalidParams,
    /// The tracker failed to serve an announce request for an unspecified
    /// reason
    Internal,
    /// The IP address of the request doesn't match the previous announce, and
    /// no `key` or a wrong one was passed as verification.
    IpAddressChanged,
    /// The torrent was not found by tracker.
    TorrentNotFound,
    /// A custom error for Extensions to use
    Custom(&'static str),
}

impl Error {
    /// Returns a human readable error message for this error,
    /// All error messages only contain characters in the ASCII printable range.
    #[inline]
    pub const fn message(&self) -> &'static str {
        match self {
            Error::AccessDenied => "access denied",
            Error::InvalidAnnounceUrl => "invalid announce URL",
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
