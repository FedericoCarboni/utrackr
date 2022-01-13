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

#[derive(Debug, Deserialize, Serialize)]
pub struct TrackerConfig {
  /// Duration, in seconds that the clients should wait for before announcing
  /// again.
  #[serde(default = "default_interval")]
  pub interval: i32,
  /// Duration, in seconds that the clients should wait for before asking for
  /// more peers. Announces will still be allowed, but an empty peer list will
  /// be returned.
  #[serde(default = "default_min_interval")]
  pub min_interval: i32,
  /// Duration, in seconds that the tracker should wait for before removing peers from the swarm
  #[serde(default = "default_max_interval")]
  pub max_interval: i32,

  /// Default number of peers for each announce request, defaults to `32`
  #[serde(default = "default_default_num_want")]
  pub default_num_want: i32,
  /// Maximum number of peers that will be put in peers, defaults to `128`
  #[serde(default = "default_max_num_want")]
  pub max_num_want: i32,

  /// Track torrents that are not already in the tracker's store. This is
  /// useful when using tracker without a database.
  #[serde(default)]
  pub track_unknown_torrents: bool,

  /// **Always** trust the self-declared IP address of the peer. This is not a
  /// good idea; there are all sorts of ways this could create problems, an
  /// attacker could announce a victim's IP address to launch a DDOS attack
  /// for example.
  ///
  /// **Note:** the tracker doesn't support DNS names in the IP parameter, it
  /// will only parse valid IPv4 and IPv6 strings.
  ///
  /// This option is **not** recommended for most use cases, but it may be
  /// useful for debugging.
  ///
  /// **Enable this option at your own risk.**
  #[serde(default)]
  pub unsafe_trust_ip_param: bool,

  /// Trust the self-declared IP address of the peer if the request came from
  /// a local address.
  ///
  /// **Note:** the tracker doesn't support DNS names in the IP parameter, it
  /// will only parse valid IPv4 and IPv6 strings.
  ///
  /// **Note:** The `ip` parameter of UDP announces doesn't support IPv6.
  ///
  /// The technical definition of *local* depends on the IP protocol used.
  ///
  /// On IPv4 the IP parameter will be trusted if the request came from an
  /// RFC 1918 private address.
  ///
  /// On IPv6 the IP parameter will be trusted if the request came from an
  /// RFC 4193 unique local address.
  #[serde(default)]
  pub trust_ip_param_if_local: bool,

  /// Deny all IP address changes. By default the tracker will allow clients
  /// to change their IP if they specify a `key` to prove their identity. This
  /// option will disable the default behavior and will uncoditionally reject
  /// announce requests if the IP address of the peer doesn't match.
  #[serde(default)]
  pub deny_all_ip_changes: bool,
}

impl Default for TrackerConfig {
  fn default() -> Self {
    Self {
      interval: default_interval(),
      min_interval: default_min_interval(),
      max_interval: default_max_interval(),

      default_num_want: default_default_num_want(),
      max_num_want: default_max_num_want(),

      track_unknown_torrents: false,
      unsafe_trust_ip_param: false,
      trust_ip_param_if_local: false,
      deny_all_ip_changes: false,
    }
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
pub struct DatabaseConfig {}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Config<T: Default> {
  #[serde(default)]
  pub tracker: TrackerConfig,
  #[serde(default, flatten)]
  pub extensions: T,
  // #[serde(default)]
  // pub http: HttpConfig,
  #[serde(default)]
  pub udp: UdpConfig,
  #[cfg(feature = "database")]
  pub database: DatabaseConfig,
}
