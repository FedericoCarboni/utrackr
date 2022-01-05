#[derive(Debug)]
pub struct TrackerConfig {
    /// Whether the tracker should accept announce requests for unknown torrents
    pub enable_unknown_torrents: bool,
    /// Whether the tracker should insert unknown torrents to the database.
    /// This does nothing if no database is enabled.
    #[cfg(feature = "database")]
    pub insert_unknown_torrents: bool,

    /// Database save interval, in seconds
    #[cfg(feature = "database")]
    pub autosave_interval: u64,

    /// How often clients should announce themselves
    pub interval: u64,
    /// How long clients should wait before announcing
    pub min_interval: u64,
    /// How long the tracker should wait before removing a peer from the swarm,
    /// defaults to 2x interval
    pub max_interval: u64,

    /// Default number of peers for each announce request, defaults to 32
    pub default_numwant: i32,
    /// Maximum number of peers that will be put in peers, defaults to 128
    pub max_numwant: i32,

    /// Whether to ignore the `ip` announce parameter, defaults to true
    pub ignore_ip_param: bool,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            /// Whether the tracker should accept announce requests for unknown torrents
            enable_unknown_torrents: true,
            /// Whether the tracker should insert unknown torrents to the database.
            /// This does nothing if no database is enabled.
            #[cfg(feature = "database")]
            insert_unknown_torrents: true,

            /// Database save interval, in seconds
            /// Does nothing if no database is enabled
            #[cfg(feature = "database")]
            autosave_interval: 60,

            /// How often clients should announce themselves, defaults to 1800
            interval: 1800,
            /// How long clients should wait before announcing
            min_interval: 900,
            /// How long the tracker should wait before removing a peer from the swarm,
            /// defaults to 3600
            max_interval: 3600,

            /// Default number of peers for each announce request, defaults to 32
            default_numwant: 32,
            /// Maximum number of peers that will be put in peers, defaults to 128
            max_numwant: 128,

            /// Whether to ignore the `ip` announce parameter, defaults to true
            ignore_ip_param: true,
        }
    }
}

pub struct HttpConfig {
    /// Enable or disable compact HTTP peer list, defaults to true
    pub compact: bool,
    /// Enable BEP 0007 compact IPv6 peer list, defaults to true
    pub compact_peers6: bool,
    /// Disallow clients from making requests with compact=0, defaults to false
    pub compact_only: bool,
    /// Disallow compact=0 requests unless IPv6, defaults to false
    pub compact_only_except_ipv6: bool,

    /// Whether to compress responses with GZIP
    pub gzip: bool,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            compact: true,
            compact_peers6: true,
            compact_only: false,
            compact_only_except_ipv6: false,
            gzip: true,
        }
    }
}

pub struct DatabaseConfig {}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {}
    }
}

pub struct Config {
    pub tracker: TrackerConfig,
    pub http: HttpConfig,
    #[cfg(feature = "database")]
    pub database: DatabaseConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tracker: Default::default(),
            http: Default::default(),
            #[cfg(feature = "database")]
            database: Default::default(),
        }
    }
}
