use crate::core::{
    announce::AnnounceParams,
    params::{EmptyParamsParser, ParamsParser},
    swarm::Peer,
    Error,
};

/// An extension for the tracker.
pub trait TrackerExtension<Config = (), Params = (), P = EmptyParamsParser>
where
    Params: Sync + Send,
    P: ParamsParser<Params>,
{
    /// Create a new parameters parser
    fn get_params_parser(&self) -> P;
    /// Validate an announce request
    #[inline]
    fn validate(&self, _: &AnnounceParams, _: &Params, _: Option<&Peer>) -> Result<(), Error> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct NoExtension;

impl TrackerExtension for NoExtension {
    #[inline]
    fn get_params_parser(&self) -> EmptyParamsParser {
        EmptyParamsParser
    }
}
