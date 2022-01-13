pub mod serde_pem {
    use serde::{Serializer, Deserialize, Deserializer, de};

    pub fn serialize<S: Serializer, T>(_: T, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str("")
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 32], D::Error> {
        let b64 = String::deserialize(deserializer)?;
        let mut s = [0; 32];
        base64::decode_config_slice(b64, base64::STANDARD, &mut s).map_err(de::Error::custom)?;
        Ok(s)
    }
}
