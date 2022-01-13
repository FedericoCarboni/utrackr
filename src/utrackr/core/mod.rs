mod announce;
mod config;
mod error;
pub mod extensions;
mod params;
pub(crate) mod query;
mod swarm;
mod tracker;

pub use announce::AnnounceParams;
pub use params::{ParamsParser, EmptyParamsParser};
pub use config::*;
pub use error::Error;
pub use swarm::*;
pub use tracker::Tracker;

/// This is a hard-coded maximum value for the number of peers that can be
/// returned in an ANNOUNCE response.
pub const MAX_NUM_WANT: usize = 256;
