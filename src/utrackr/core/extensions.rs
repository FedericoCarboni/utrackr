use crate::core::{
    announce::AnnounceParams,
    params::{EmptyParseParamsExt, ParseParamsExt},
    swarm::Peer,
    Error,
};

pub trait TrackerExt<Q = (), P = EmptyParseParamsExt>
where
    P: ParseParamsExt<Q>,
{
    /// Get a parameter parser extension
    fn params(&self) -> P;
    /// Validate an announce request
    #[inline]
    fn validate(&self, _: &AnnounceParams<Q>, _: Option<&Peer>) -> Result<(), Error> {
        Ok(())
    }
}
