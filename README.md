# utrackr
Hackable peformance-focused BitTorrent tracker. Currently only implements the UDP Tracker Protocol[^1].

The tracker supports UDP extensions[^2] and partial seeds[^3].

# Building
For the moment, if you want to try this tracker you'll have to compile it from
source. The tracker itself is 100% Rust, but its dependencies are not. Besides
Rust/Cargo you will also need a C/C++ compiler, required because of
[ring's build requirements](https://github.com/briansmith/ring/blob/main/BUILDING.md).
Run the following command and wait for it to complete. The executable will be
created in `target/release`.

```
cargo build --release
```

[^1]: [BEP 15, UDP Tracker Protocol for BitTorrent](https://www.bittorrent.org/beps/bep_0015.html)
[^2]: [BEP 41, UDP Tracker Protocol Extensions](https://www.bittorrent.org/beps/bep_0041.html)
[^3]: [BEP 21, Extension for partial seeds](https://www.bittorrent.org/beps/bep_0021.html)
