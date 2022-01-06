use std::io;
#[cfg(windows)]
use std::{
    mem,
    net::{AsRawSocket, UdpSocket},
};

#[cfg(windows)]
#[inline]
pub fn unset_ipv6_v6only(socket: impl AsRawSocket) -> io::Result<()> {
    // TODO: windows set IPV6_V6ONLY to 1 by default, meaning we won't be able
    // to use the same socket to interact with both IPv4 and IPv6 clients.
    let raw_socket = socket.as_raw_socket();
    let value = 0 as libc::c_int;
    let result = unsafe {
        libc::setsockopt(
            raw_socket,
            libc::IPPROTO_IPV6,
            libc::IPV6_V6ONLY,
            &value as *const libc::c_void,
            mem::size_of::<libc::c_int>(),
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(windows))]
#[inline]
pub fn unset_ipv6_v6only<T>(_: &T) -> io::Result<()> {
    Ok(())
}
