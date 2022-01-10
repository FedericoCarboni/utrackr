use std::{
    fmt, io,
    net::{SocketAddr, ToSocketAddrs},
};

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

pub struct BindAddrs {
    addrs: Vec<SocketAddr>,
}

impl BindAddrs {
    pub fn addrs(&self) -> &[SocketAddr] {
        &self.addrs
    }
}

impl fmt::Debug for BindAddrs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.addrs.len() == 1 {
            self.addrs[0].fmt(f)
        } else {
            self.addrs.fmt(f)
        }
    }
}

impl Default for BindAddrs {
    fn default() -> Self {
        Self {
            addrs: vec![SocketAddr::from(([0; 16], 6969))],
        }
    }
}

impl<T: ToSocketAddrs> From<&T> for BindAddrs {
    fn from(addrs: &T) -> Self {
        Self {
            addrs: addrs
                .to_socket_addrs()
                .expect("failed to convert to BindAddrs")
                .collect(),
        }
    }
}

impl ToSocketAddrs for BindAddrs {
    type Iter = <Vec<SocketAddr> as IntoIterator>::IntoIter;

    fn to_socket_addrs(&self) -> io::Result<Self::Iter> {
        Ok(self.addrs.clone().into_iter())
    }
}

impl<'de> Deserialize<'de> for BindAddrs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Value<'a> {
            Str(&'a str),
            StrVec(Vec<&'a str>),
        }
        match Value::deserialize(deserializer)? {
            Value::Str(s) => Ok(Self {
                addrs: vec![s.parse::<SocketAddr>().map_err(de::Error::custom)?],
            }),
            Value::StrVec(s) => {
                if s.is_empty() {
                    return Err(de::Error::invalid_length(s.len(), &">=1"));
                }
                Ok(Self {
                    addrs: s
                        .iter()
                        .map(|s| s.parse().map_err(de::Error::custom))
                        .collect::<Result<Vec<SocketAddr>, D::Error>>()?,
                })
            }
        }
    }
}

impl Serialize for BindAddrs {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.addrs.len() == 1 {
            serializer.collect_str(&self.addrs[0].to_string())
        } else {
            serializer.collect_seq(self.addrs.iter().map(|addr| addr.to_string()))
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TrackerConfig {
    /// Whether the tracker should accept announce requests for unknown torrents
    #[serde(default)]
    pub announce_unknown_torrents: bool,

    /// How often clients should announce themselves
    #[serde(default = "TrackerConfig::default_interval")]
    pub interval: i32,
    /// How long clients should be forced to wait before announcing again
    #[serde(default = "TrackerConfig::default_min_interval")]
    pub min_interval: i32,
    /// How long the tracker should wait before removing a peer from the swarm,
    /// defaults to 1800
    #[serde(default = "TrackerConfig::default_max_interval")]
    pub max_interval: i32,

    /// Default number of peers for each announce request, defaults to 32
    #[serde(default = "TrackerConfig::default_default_num_want")]
    pub default_num_want: i32,
    /// Maximum number of peers that will be put in peers, defaults to 128
    #[serde(default = "TrackerConfig::default_max_num_want")]
    pub max_num_want: i32,

    /// Whether to honor the `ip` announce parameter, defaults to false.
    /// When this option is enabled the tracker will not be able to verify that
    /// IP addresses are correct.
    #[serde(default)]
    pub unsafe_honor_ip_param: bool,
    /// Wheather the tracker should use the ip parameter for announce requests
    /// coming from local ip addresses.
    #[serde(default)]
    pub honor_ip_param_if_local: bool,
    // #[serde(default)]
    // pub ed25519: Option<u8>,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            announce_unknown_torrents: false,

            interval: TrackerConfig::default_interval(),
            min_interval: TrackerConfig::default_min_interval(),
            max_interval: TrackerConfig::default_max_interval(),

            default_num_want: TrackerConfig::default_default_num_want(),
            max_num_want: TrackerConfig::default_max_num_want(),

            unsafe_honor_ip_param: false,
            honor_ip_param_if_local: false,
            // ed25519: None,
        }
    }
}

impl TrackerConfig {
    fn default_interval() -> i32 {
        900
    }
    fn default_min_interval() -> i32 {
        60
    }
    fn default_max_interval() -> i32 {
        1800
    }
    fn default_default_num_want() -> i32 {
        32
    }
    fn default_max_num_want() -> i32 {
        128
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct HttpConfig {
    /// Enable or disable the HTTP tracker
    #[serde(default)]
    pub disable: bool,
    #[serde(default)]
    pub bind: BindAddrs,
    /// Enable or disable compact HTTP peer list, defaults to true
    #[serde(default)]
    pub disable_compact_peers: bool,
    /// Enable BEP 07 compact IPv6 peer list, defaults to true
    #[serde(default)]
    pub disable_compact_peers6: bool,
    /// Disallow clients from making requests with compact=0, defaults to false
    #[serde(default)]
    pub compact_only: bool,
    /// Disallow compact=0 requests unless IPv6, incompatible with `compact_only`.
    #[serde(default)]
    pub compact_only_except_ipv6: bool,
    #[serde(default)]
    pub include_peer_id: bool,

    /// Whether to compress responses with GZIP
    #[serde(default)]
    pub disable_gzip: bool,
    #[serde(default)]
    pub disable_bzip2: bool,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct UdpConfig {
    #[serde(default)]
    pub disable: bool,
    #[serde(default)]
    pub bind: BindAddrs,
    #[serde(default)]
    pub ipv6_only: bool,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct DatabaseConfig {

}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub tracker: TrackerConfig,
    // #[serde(default)]
    // pub http: HttpConfig,
    #[serde(default)]
    pub udp: UdpConfig,
    #[cfg(feature = "database")]
    pub database: DatabaseConfig,
}
