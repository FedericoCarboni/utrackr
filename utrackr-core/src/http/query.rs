use std::net::SocketAddr;

pub(crate) struct AnnounceQuery {
    pub(crate) info_hash: [u8; 20],
    pub(crate) peer_id: [u8; 20],
    pub(crate) key: u32,
    pub(crate) addr: SocketAddr,
    pub(crate) compact: bool,
    pub(crate) numwant: i32,
}

pub(crate) fn parse_announce(query: &[u8], remote_addr: &SocketAddr) -> Option<AnnounceQuery> {
    let mut info_hash = [0u8; 20];
    let mut peer_id = [0u8; 20];
    let mut key: Option<u32> = None;
    let mut addr = *remote_addr;
    let mut compact = true;
    let mut numwant = -1i32;

    let mut query = &query[..];

    while !query.is_empty() {
        let end = query.iter().position(|&c| c == b'=').unwrap_or_else(|| query.len());
        let mut key = &query[..end];
        query = &query[end..];
        let end = query.iter().position(|&c| c == b'&').unwrap_or_else(|| query.len());
        let mut value = &query[..end];
        query = &query[end..];
    }

    match key {
        Some(key) => Some(AnnounceQuery {
            info_hash: info_hash,
            peer_id,
            key,
            addr,
            compact,
            numwant,
        }),
        _ => None,
    }
}
