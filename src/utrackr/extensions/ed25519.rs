use std::marker::PhantomData;

use ring::signature::{VerificationAlgorithm, ED25519};
use serde::{de, Deserialize, Deserializer, Serialize};

use crate::core::{
  extensions::{NoExtension, TrackerExtension},
  AnnounceParams, EmptyParamsParser, Error, ParamsParser, Peer,
};

pub fn b64deserialize<'de, D: Deserializer<'de>>(
  deserializer: D,
) -> Result<[u8; 32], D::Error> {
  let b64 = String::deserialize(deserializer)?;
  let mut s = [0; 32];
  base64::decode_config_slice(b64, base64::STANDARD, &mut s)
    .map_err(de::Error::custom)?;
  Ok(s)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Encoding {
  #[serde(rename = "base64")]
  Base64,
  // #[serde(rename = "hex")]
  // Hex,
  // #[serde(rename = "url")]
  // Url,
}

impl Default for Encoding {
  fn default() -> Self {
    Self::Base64
  }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Ed25519Config {
  #[serde(default)]
  param_name: String,
  #[serde(default, rename = "encoding")]
  _encoding: Encoding,
  #[serde(deserialize_with = "b64deserialize")]
  public_key: [u8; 32],
}

#[derive(Debug, Default, Deserialize)]
pub struct Ed25519ConfigExt<T> {
  #[serde(default)]
  ed25519: Option<Ed25519Config>,
  #[serde(flatten)]
  _extension: T,
}

#[derive(Debug)]
pub struct Ed25519Params<Params> {
  verify: Option<[u8; 64]>,
  params: Params,
}

#[derive(Debug)]
pub struct Ed25519ParamsParser<Params, P: ParamsParser<Params>> {
  param_name: Option<([u8; 32], usize)>,
  verify: Option<[u8; 64]>,
  parser: P,
  _marker: PhantomData<Params>,
}

impl<Params, P: ParamsParser<Params>> TryInto<Ed25519Params<Params>>
  for Ed25519ParamsParser<Params, P>
{
  type Error = Error;

  fn try_into(self) -> Result<Ed25519Params<Params>, Self::Error> {
    Ok(Ed25519Params {
      verify: self.verify,
      params: self.parser.try_into()?,
    })
  }
}

impl<Params, P: ParamsParser<Params>> ParamsParser<Ed25519Params<Params>>
  for Ed25519ParamsParser<Params, P>
{
  fn parse(&mut self, key: &[u8], value: &[u8]) -> Result<(), Error> {
    if let Some((param_name, len)) = self.param_name {
      if key == &param_name[..len] {
        if self.verify.is_some() || value.len() != 86 {
          return Err(Error::InvalidParams);
        }
        let mut decoded_value = [0u8; 64];
        base64::decode_config_slice(
          value,
          base64::URL_SAFE_NO_PAD,
          &mut decoded_value,
        )
        .map_err(|_| Error::InvalidParams)?;
        self.verify = Some(decoded_value);
      }
    } else {
      self.parser.parse(key, value)?;
    }
    Ok(())
  }
}

#[derive(Debug)]
pub struct Ed25519<E = NoExtension, C = (), P = (), D = EmptyParamsParser>
where
  E: TrackerExtension<P, D>,
  P: Sync + Send,
  D: ParamsParser<P> + Sync + Send,
{
  config: Ed25519ConfigExt<C>,
  extension: E,
  _marker: PhantomData<(P, D)>,
}

impl<E, C, P, D> Ed25519<E, C, P, D>
where
  E: TrackerExtension<P, D>,
  P: Sync + Send,
  D: ParamsParser<P> + Sync + Send,
{
  #[inline]
  pub fn with_extension(extension: E, config: Ed25519ConfigExt<C>) -> Self {
    Self {
      config,
      extension,
      _marker: PhantomData,
    }
  }
}

impl<E, C, P, D> TrackerExtension<Ed25519Params<P>, Ed25519ParamsParser<P, D>>
  for Ed25519<E, C, P, D>
where
  E: TrackerExtension<P, D>,
  C: Sync + Send,
  P: Sync + Send,
  D: ParamsParser<P> + Sync + Send,
{
  fn get_params_parser(&self) -> Ed25519ParamsParser<P, D> {
    Ed25519ParamsParser {
      param_name: self.config.ed25519.as_ref().map(|config| {
        let mut param_name = [0; 32];
        param_name[..config.param_name.len()]
          .copy_from_slice(config.param_name.as_bytes());
        (param_name, config.param_name.len())
      }),
      verify: None,
      parser: self.extension.get_params_parser(),
      _marker: PhantomData,
    }
  }

  fn validate(
    &self,
    announce: &AnnounceParams,
    params: &Ed25519Params<P>,
    peer: Option<&Peer>,
  ) -> Result<(), Error> {
    if let Some(config) = self.config.ed25519.as_ref() {
      if let Some(verify) = params.verify.as_ref() {
        ED25519
          .verify(
            untrusted::Input::from(&config.public_key),
            untrusted::Input::from(announce.info_hash()),
            untrusted::Input::from(verify),
          )
          .map_err(|_| Error::TorrentNotFound)?;
      } else {
        return Err(Error::TorrentNotFound);
      }
    }
    self.extension.validate(announce, &params.params, peer)
  }
}
