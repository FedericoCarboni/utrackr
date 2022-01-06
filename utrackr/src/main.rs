use std::{io::BufReader, io::prelude::*, fs::File};

use utrackr_core::{Tracker, Config};
use utrackr_udp::UdpTracker;

#[tokio::main]
async fn main() {
    env_logger::init();

    let mut f = File::open("utrackr.toml").unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();

    let cfg: Config = toml::from_str(&s).unwrap();
    // let mut f = File::create("utrackr.toml").unwrap();

    // f.write(toml::to_string_pretty(&cfg).unwrap().as_bytes()).unwrap();

    UdpTracker::bind(Tracker::new(cfg.tracker), cfg.udp).await.unwrap()
        .run_until(tokio::signal::ctrl_c()).await;
}
