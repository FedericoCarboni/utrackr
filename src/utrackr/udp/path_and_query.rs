//! UDP Tracker Protocol Extensions

pub(in crate::udp) const MAX_PATH_AND_QUERY_LEN: usize = 1020;

pub(in crate::udp) fn parse_extensions_urldata(packet: &[u8], len: usize) -> [u8; 1020] {
    let mut path_and_query = [0; 1020];
    let mut offset = 0;
    let mut i = 0;
    while i < len {
        let option_type = packet[i];
        if option_type >= 2 {
            i += 1;
            if i >= len {
                // this is a lenient parser, ignore the error
                break;
            }
            let opt_len = packet[i] as usize;
            dbg!(opt_len);
            if i + opt_len >= len {
                break;
            }
            i += 1;
            if option_type == 2 {
                path_and_query[offset..offset + opt_len].copy_from_slice(&packet[i..i + opt_len]);
                offset += opt_len;
            }
            i += opt_len;
        } else if option_type == 0 {
            break;
        } else {
            i += 1;
        }
    }
    path_and_query
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse() {
        let pkt = b"\x02\x0bhello world\x01\x02\x0bhello world\x00\x02\x0bhello world";
        println!(
            "pkt: {}",
            std::str::from_utf8(&parse_extensions_urldata(pkt, pkt.len())).unwrap()
        );
    }
}
