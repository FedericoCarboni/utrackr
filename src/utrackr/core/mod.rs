mod swarm;
mod tracker;
mod config;
pub use tracker::Tracker;
pub use swarm::*;
pub use config::*;
mod error;
pub mod params;
pub mod query;
pub use error::Error;
pub mod announce;
pub mod extensions;

/// This is a hard-coded maximum value for the number of peers that can be
/// returned in an ANNOUNCE response.
pub const MAX_NUM_WANT: usize = 256;
