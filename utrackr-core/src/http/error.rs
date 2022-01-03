macro_rules! bencode {
    ($($key:literal => $value:literal),*) => {{
        let mut bencoded = Vec::<u8>::new();
        bencoded.push(b'd');
        let mut offset = 1;
        $({
            let key = $key.as_bytes();
            bencoded.extend_from_slice(key.len().to_string().as_bytes());
            bencoded.push(b':');
            bencoded.extend_from_slice(key);
            let value = $value.as_bytes();
            bencoded.extend_from_slice(value.len().to_string().as_bytes());
            bencoded.push(b':');
            bencoded.extend_from_slice(value);
        })*
        bencoded.push(b'e');
        bencoded
    }};
}

pub(crate) const INVALID_INFO_HASH: &[u8] = b"d14:failure reason17:invalid info hashe";
pub(crate) const UNKNOWN_500: &[u8] = b"d14:failure reason21:internal server errore";
pub(crate) const ACCESS_DENIED: &[u8] = b"d14:failure reason13:access deniede";

#[cfg(test)]
mod test {
    #[test]
    fn test_err() {
        let x = bencode! {
            "failure reason" => "access denied",
        };
        println!("{}", String::from_utf8_lossy(x));
    }
}
