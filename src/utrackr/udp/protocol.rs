use std::net::IpAddr;

use arrayref::array_ref;
use ring::digest;

use crate::core::MAX_NUM_WANT;

/// XBT Tracker uses 2048, opentracker uses 8192, it could be tweaked for
/// performance reasons
pub(in crate::udp) const MAX_PACKET_SIZE: usize = 8192;
/// CONNECT is the smallest packet in the protocol
pub(in crate::udp) const MIN_PACKET_SIZE: usize = MIN_CONNECT_SIZE;

/// The secret is used generate `connection_id`, to prevent UDP sender address
/// spoofing. 8 bytes should be enough, if an attacker has to guess 8 bytes they
/// might as well try to guess the `connection_id` itself.
pub(in crate::udp) type Secret = [u8; 8];

/// This is a hard-coded maximum value for the number of torrents that can be
/// scraped with a single UDP packet.
/// BEP 15 states `Up to about 74 torrents can be scraped at once. A full scrape
/// can't be done with this protocol.`
/// If clients need to scrape more torrents they can just send more than one
/// SCRAPE packet.
pub(in crate::udp) const MAX_SCRAPE_TORRENTS: usize = 128;

pub const MIN_CONNECT_SIZE: usize = 16;
pub const MIN_ANNOUNCE_SIZE: usize = 98;
pub const MIN_SCRAPE_SIZE: usize = 36;

pub const CONNECT_SIZE: usize = 16;
pub const ANNOUNCE_SIZE: usize = 20 + 18 * MAX_NUM_WANT;
pub const SCRAPE_SIZE: usize = 8 + 12 * MAX_SCRAPE_TORRENTS;

pub const PROTOCOL_ID: [u8; 8] = 0x41727101980i64.to_be_bytes();

pub const ACTION_CONNECT: [u8; 4] = 0x0i32.to_be_bytes();
pub const ACTION_ANNOUNCE: [u8; 4] = 0x1i32.to_be_bytes();
pub const ACTION_SCRAPE: [u8; 4] = 0x2i32.to_be_bytes();
// const ACTION_ERROR: [u8; 4] = 0x3i32.to_be_bytes();

#[inline(always)]
fn copy_from_slice<T: Copy>(a: &mut [T], b: &[T]) {
    debug_assert_eq!(a.len(), b.len());
    for i in 0..a.len() {
        a[i] = b[i];
    }
}

#[inline]
fn ip_to_bytes(ip: &IpAddr) -> [u8; 16] {
    match ip {
        IpAddr::V4(ipv4) => ipv4.to_ipv6_mapped().octets(),
        IpAddr::V6(ipv6) => ipv6.octets()
    }
}

/// The UDP Tracker Protocol specification recommends that connection ids have
/// two properties:
///  - they should not be guessable by clients
///  - they should be accepted for at least 2 minutes after they're generated
/// `connection_id` is the first 8 bytes of the SHA-2 hash of `secret`,
/// `two_min_window` and `ip`.
#[inline]
fn make_connection_id(secret: &Secret, two_min_window: u64, remote_ip: &[u8; 16]) -> [u8; 8] {
    let mut data = [0u8; 32];
    data[0..8].copy_from_slice(secret);
    data[8..16].copy_from_slice(&two_min_window.to_be_bytes());
    data[16..32].copy_from_slice(remote_ip);
    let sha2 = digest::digest(&digest::SHA256, &data);
    // connection_id is only 8 bytes
    *array_ref!(sha2.as_ref(), 0, 8)
}

#[inline]
fn verify_connection_id(
    secret: &[u8; 8],
    time_frame: u64,
    ip: &IpAddr,
    connection_id: &[u8; 8],
) -> bool {
    let ip_bytes = ip_to_bytes(ip);
    *connection_id == make_connection_id(secret, time_frame, &ip_bytes)
        || *connection_id == make_connection_id(secret, time_frame - 1, &ip_bytes)
}

#[inline]
pub(in crate::udp) fn connect(transaction_id: &[u8; 4], ip: &IpAddr, time: u64, secret: &Secret) -> [u8; CONNECT_SIZE] {
    let mut rpkt = [0; CONNECT_SIZE];
    copy_from_slice(&mut rpkt[0..4], &ACTION_CONNECT);
    copy_from_slice(&mut rpkt[4..8], transaction_id);
    copy_from_slice(&mut rpkt[8..16], &make_connection_id(secret, time, &ip_to_bytes(ip)));
    rpkt
}
