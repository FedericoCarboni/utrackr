use crate::core::{
    params::ParamsParser,
    query::{decode_percent_byte, QueryParser},
    Error,
};

const OPTION_TYPE_END: u8 = 0x0;
const OPTION_TYPE_URLDATA: u8 = 0x2;

#[derive(Debug, Clone)]
enum OptionType<'a> {
    UrlData(&'a [u8]),
}

#[derive(Debug, Clone)]
struct OptionsIter<'a> {
    index: usize,
    packet: &'a [u8],
}

impl<'a> OptionsIter<'a> {
    #[inline]
    fn next_u8(&mut self) -> Option<u8> {
        let v = self.packet.get(self.index)?;
        self.index += 1;
        Some(*v)
    }
}

impl<'a> Iterator for OptionsIter<'a> {
    type Item = OptionType<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let option_type = self.next_u8()?;
        if option_type >= OPTION_TYPE_URLDATA {
            let len = self.next_u8()?;
            if len == 0 {
                return self.next();
            }
            let len = len as usize;
            if self.index + len > self.packet.len() {
                return None;
            }
            let slice = &self.packet[self.index..self.index + len];
            self.index += len;
            // The protocol may be extended with more option types in the future
            if option_type == OPTION_TYPE_URLDATA {
                return Some(OptionType::UrlData(slice));
            }
        } else if option_type == OPTION_TYPE_END {
            return None;
        } else {
            // Option type nop does nothing, it is just padding
        }
        self.next()
    }
}

/// Returns true if iter starts with "/announce" (also decodes url escapes).
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
            // "%61%6e%6e%6f%75%6e%63%65" should still be
            // interpreted as "announce"
            let b = if b == b'%' {
                decode_percent_byte(iter).unwrap_or(b)
            } else {
                b
            };
            if b != expected {
                return false;
            }
        } else {
            // The uri ended too soon
            return false;
        }
    }
    true
}

/// Parses BEP 41 extensions and parses the query using `parser`, the path part
/// of the request string MUST be `/announce`.
///
/// https://www.bittorrent.org/beps/bep_0041.html#extension-format
pub fn parse_extensions<T, P>(mut parser: P, packet: &[u8]) -> Result<T, Error>
where
    P: ParamsParser<T>,
{
    // If the extension part of the packet is empty or starts with a zero then
    // we assume the client doesn't support BEP 41.
    if !packet.is_empty() && packet[0] != 0 {
        let mut iter = OptionsIter { index: 0, packet }.peekable();
        // If there are no known options then we treat the request as if it
        // didn't include any extensions
        if iter.peek().is_none() {
            return parser.try_into();
        }
        let mut iter = iter.flat_map(|OptionType::UrlData(v)| v.iter());
        if !starts_with_announce(&mut iter) {
            // If the client sends a BEP 41 announce, only "/announce" (and
            // optionally query parameters) will be served. Other URLs will
            // error out.
            return Err(Error::InvalidAnnounceUrl);
        }
        // "/announce" can only be followed by a '?' + query parameters.
        if let Some(&b) = iter.next() {
            if b != b'?' {
                return Err(Error::InvalidAnnounceUrl);
            }
            let mut query_parser = QueryParser::new(iter);
            while let Some((key, value)) = query_parser.next() {
                parser.parse(key, value)?;
            }
        }
    }
    // Custom parameter parsers are expected to deal with the absence of query
    // parameters.
    parser.try_into()
}
