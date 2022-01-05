// Copyright 2013-2016 The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Parser and serializer for the [`application/x-www-form-urlencoded` syntax](
//! http://url.spec.whatwg.org/#application/x-www-form-urlencoded), specialized
//! for use in BitTorrent trackers. The `form_urlencoded` crate decodes all keys
//! and values as UTF-8, causing errors with binary parameters used in the
//! BitTorrent tracker protocol. All functions return byte strings instead of
//! string slices.
//!
//! Converts between a string (such as an URL’s query string)
//! and a sequence of (name, value) pairs.

use std::borrow::Cow;
use percent_encoding::percent_decode;

/// Convert a byte string in the `application/x-www-form-urlencoded` syntax
/// into a iterator of (name, value) pairs.
///
/// Use `parse(input.as_bytes())` to parse a `&str` string.
///
/// The names and values are percent-decoded. For instance, `%23first=%25try%25` will be
/// converted to `[("#first", "%try%")]`.
#[inline]
pub(crate) fn parse(input: &[u8]) -> UrlDecode<'_> {
    UrlDecode { input }
}

/// The return type of `parse()`.
#[derive(Copy, Clone)]
pub(crate) struct UrlDecode<'a> {
    input: &'a [u8],
}

impl<'a> Iterator for UrlDecode<'a> {
    type Item = (Cow<'a, [u8]>, Cow<'a, [u8]>);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.input.is_empty() {
                return None;
            }
            let mut split2 = self.input.splitn(2, |&b| b == b'&');
            let sequence = split2.next().unwrap_or(&[][..]);
            self.input = split2.next().unwrap_or(&[][..]);
            if sequence.is_empty() {
                continue;
            }
            let mut split2 = sequence.splitn(2, |&b| b == b'=');
            let name = split2.next().unwrap_or(&[][..]);
            let value = split2.next().unwrap_or(&[][..]);
            return Some((decode(name), decode(value)));
        }
    }
}

fn decode(input: &[u8]) -> Cow<[u8]> {
    let replaced = replace_plus(input);
    match percent_decode(&replaced).into() {
        Cow::Owned(vec) => Cow::Owned(vec),
        Cow::Borrowed(_) => replaced,
    }
}

/// Replace b'+' with b' '
fn replace_plus(input: &[u8]) -> Cow<'_, [u8]> {
    match input.iter().position(|&b| b == b'+') {
        None => Cow::Borrowed(input),
        Some(first_position) => {
            let mut replaced = input.to_owned();
            replaced[first_position] = b' ';
            for byte in &mut replaced[first_position + 1..] {
                if *byte == b'+' {
                    *byte = b' ';
                }
            }
            Cow::Owned(replaced)
        }
    }
}
