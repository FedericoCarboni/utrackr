use std::{pin::Pin, future::{Future, ready}};

use crate::core::{
    announce::AnnounceParams,
    params::{EmptyParamsParser, ParamsParser},
    swarm::Peer,
    Error,
};

/// An extension for the tracker.
pub trait TrackerExtension<Config = (), Params = (), P = EmptyParamsParser>
where
    P: ParamsParser<Params>,
{
    /// Create a new parameters parser
    fn get_params_parser(&self) -> P;
    /// Validate an announce request
    #[inline]
    fn validate(&self, _: &AnnounceParams<Params>, _: Option<&Peer>) -> Result<(), Error> {
        Ok(())
    }
}
