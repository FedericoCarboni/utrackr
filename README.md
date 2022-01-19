# utrackr
Hackable peformance-focused BitTorrent tracker.

Currently only the UDP Tracker Protocol[^1] is implemented, with support for UDP
extensions[^2].

The tracker stores swarms in-memory only, IP addresses are never saved anywhere
and they will expire after a set time. IPv6 peers and partial seeds[^3] are also
supported. At the moment databases are not supported (see issue
[#8](https://github.com/FedericoCarboni/utrackr/issues/8)), so statistics are
not accurate.

# IPv6 support
By default (on *nix, Windows has a different behavior unfortunately, see issue
[#9](https://github.com/FedericoCarboni/utrackr/issues/9)), the tracker serves
both IPv4 and IPv6 peers, and matches announces from peers that announce with
both protocols.

**Note:** While IPv4 announces will only include IPv4 addresses, IPv6 announces
may also include IPv4 addresses mapped to IPv6.

# Building
For the moment, if you want to try this tracker you'll have to compile it from
source. The tracker itself is 100% Rust, but its dependencies are not. Besides
Rust and Cargo you will also need a C/C++ and assembly compiler, required
because of [Ring's build requirements][ring-building].

To build the tracker, run the following command and wait for it to complete.

```
cargo build --release
```

The executable will be created in `target/release`. The output executable is
linked statically to all of its dependencies.

[ring-building]: https://github.com/briansmith/ring/blob/main/BUILDING.md "Ring's building requirements"

[^1]: [BEP 15, UDP Tracker Protocol for BitTorrent](https://www.bittorrent.org/beps/bep_0015.html)
[^2]: [BEP 41, UDP Tracker Protocol Extensions](https://www.bittorrent.org/beps/bep_0041.html)
[^3]: [BEP 21, Extension for partial seeds](https://www.bittorrent.org/beps/bep_0021.html)
