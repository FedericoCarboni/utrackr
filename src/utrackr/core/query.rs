/// A low-level query parameter parser. We cannot use the `form_urlencoded` crate
/// because it requires query parameters to be valid UTF-8; the BitTorrent Tracker
/// specification requires binary data to be in urlencoded form so the parser would
/// choke on valid input.
///
/// This parser doesn't allocate anything, everything is stored on the stack.
/// However this introduces a few limitations: keys are limited to 32 bytes, and
/// values to 256 (in decoded form). Extra bytes will be cut off. These limits are not stable across minor
/// versions changes.
///
/// To turn a query string into a deserialized structure use a `ParseParamsExt`.
pub(crate) struct QueryParser<'a, I: Iterator<Item = &'a u8> + Clone> {
    key: [u8; 32],
    value: [u8; 256],
    input: I,
}

impl<'a, I: Iterator<Item = &'a u8> + Clone> QueryParser<'a, I> {
    #[inline]
    pub fn new(input: I) -> Self {
        Self {
            key: [0; 32],
            value: [0; 256],
            input,
        }
    }
    pub fn next(&mut self) -> Option<(&[u8], &[u8])> {
        let mut broken = false;
        let mut key_size = 0;
        while let Some(&b) = self.input.next() {
            let b = match b {
                b'%' => decode_percent_byte(&mut self.input).unwrap_or(b'%'),
                b'+' => b' ',
                b'=' => {
                    broken = true;
                    break;
                }
                b'&' => {
                    return Some((&self.key[..key_size], &[]));
                }
                b => b,
            };
            if key_size >= self.key.len() {
                continue;
            }
            self.key[key_size] = b;
            key_size += 1;
        }
        if !broken {
            return None;
        }
        let mut value_size = 0;
        while let Some(&b) = self.input.next() {
            let b = match b {
                b'%' => decode_percent_byte(&mut self.input).unwrap_or(b'%'),
                b'+' => b' ',
                b'&' => break,
                b => b,
            };
            if value_size >= self.value.len() {
                continue;
            }
            self.value[value_size] = b;
            value_size += 1;
        }
        Some((&self.key[..key_size], &self.value[..value_size]))
    }
}

#[inline]
fn to_digit(b: u8) -> Option<u8> {
    let mut digit = b.wrapping_sub(b'0');
    if digit < 10 {
        return Some(digit);
    }
    // Force the 6th bit to be set to ensure ascii is lower case.
    digit = (b | 0b10_0000).wrapping_sub(b'a').saturating_add(10);
    if digit < 16 {
        Some(digit)
    } else {
        None
    }
}

#[inline]
pub(crate) fn decode_percent_byte<'a>(iter: &mut (impl Iterator<Item = &'a u8> + Clone)) -> Option<u8> {
    let mut clone_iter = iter.clone();
    let h = to_digit(*clone_iter.next()?)?;
    let l = to_digit(*clone_iter.next()?)?;
    *iter = clone_iter;
    Some(h << 4 | l)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_query_parser() {
        let mut parser = QueryParser::new(
            b"=a+value+%without+a+key&a+key+without+a+value&i%20love%20escapes%%41%41%43=%values%20love%20escapes%20too%".iter(),
        );
        while let Some((key, value)) = parser.next() {
            println!(
                "{:?} = {:?}",
                std::str::from_utf8(key).unwrap(),
                std::str::from_utf8(value).unwrap()
            );
        }
    }
}
