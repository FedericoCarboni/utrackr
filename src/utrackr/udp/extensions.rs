//! UDP Tracker Protocol Extensions
//!

use crate::core::{
    params::ParamsParser,
    query::{decode_percent_byte, QueryParser},
    Error,
};

const OPTION_TYPE_END: u8 = 0x0;
const OPTION_TYPE_URLDATA: u8 = 0x2;

#[derive(Debug)]
enum OptionType<'a> {
    UrlData(&'a [u8]),
}

#[derive(Debug, Clone)]
struct OptionsIter<'a> {
    offset: usize,
    packet: &'a [u8],
}

impl<'a> OptionsIter<'a> {
    #[inline]
    fn next_u8(&mut self) -> u8 {
        let value = self.packet[self.offset];
        self.offset += 1;
        value
    }
    #[inline]
    fn check(&self) -> Option<()> {
        if self.offset + 1 >= self.packet.len() {
            None
        } else {
            Some(())
        }
    }
}

impl<'a> Iterator for OptionsIter<'a> {
    type Item = OptionType<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.check()?;
        let option_type = self.next_u8();
        self.check()?;
        if option_type >= OPTION_TYPE_URLDATA {
            let len = self.next_u8();
            if len == 0 {
                return self.next();
            }
            self.check()?;
            let len = len as usize;
            let slice = &self.packet[self.offset..self.offset + len];
            self.offset += len;
            if option_type == OPTION_TYPE_URLDATA {
                return Some(OptionType::UrlData(slice));
            }
        } else if option_type == OPTION_TYPE_END {
            return None;
        } else {
            // option type nop does nothing, it is just padding
        }
        self.next()
    }
}

#[inline]
fn starts_with_announce<'a>(iter: &mut (impl Iterator<Item = &'a u8> + Clone)) -> bool {
    if let Some(&b) = iter.next() {
        if b != b'/' {
            return false;
        }
    } else {
        return false;
    }
    for &expected in b"announce".iter() {
        if let Some(&b) = iter.next() {
            let b = if b == b'%' {
                decode_percent_byte(iter).unwrap_or(b)
            } else {
                b
            };
            if b != expected {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

#[inline]
pub fn parse_request<T, P>(mut parser: P, packet: &[u8]) -> Result<T, Error>
where
    P: ParamsParser<T>,
{
    let options_iter = OptionsIter { offset: 0, packet };
    let mut iter = options_iter.flat_map(|OptionType::UrlData(v)| v.iter());
    if !starts_with_announce(&mut iter) {
        return Err(Error::InvalidParams);
    }
    let mut query_parser = QueryParser::new(iter);
    while let Some((key, value)) = query_parser.next() {
        parser.parse(key, value)?;
    }
    parser.try_into()
}
