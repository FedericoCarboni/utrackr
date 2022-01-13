use std::{fs::File, io::prelude::*, sync::Arc};

use utrackr::core::{Config, Tracker};
use utrackr::udp::UdpTracker;

#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .parse_env("UTRACKR_LOG")
        .init();

    let mut f = File::open("utrackr.toml").unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();

    let cfg: Config<()> = match toml::from_str(&s) {
        Ok(cfg) => cfg,
        Err(err) => {
            log::error!("{}", err);
            panic!("{}", err);
        }
    };

    if cfg.udp.disable {
        log::error!("udp tracker disabled");
        panic!("udp tracker disabled");
    }

    let tracker = Arc::new(Tracker::new(cfg.tracker));
    // tracker.start_autosave();

    let mut udp_join_handle = if cfg.udp.disable {
        tokio::spawn(async {})
    } else {
        match UdpTracker::bind(tracker, cfg.udp).await {
            Ok(udp) => tokio::spawn(udp.run()),
            Err(err) => {
                log::error!("udp tracker failed {}", err);
                panic!("{}", err);
            }
        }
    };

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            log::info!("shutting down");
        }
        _ = &mut udp_join_handle => {}
    }
}
