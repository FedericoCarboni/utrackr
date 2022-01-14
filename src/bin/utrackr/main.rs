use std::{fs::File, io::prelude::*, sync::Arc};

use clap::{app_from_crate, arg};

use utrackr::core::{Config, Tracker};
use utrackr::extensions::ed25519::{Ed25519, Ed25519Config};
use utrackr::udp::UdpTracker;

#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .parse_env("UTRACKR_LOG")
        .init();

    let args = app_from_crate!()
        // .color(ColorChoice::Never)
        .arg(arg!(-c --config [CONFIG] "Optionally sets a config file to use"))
        .get_matches();

    let config: Config<Ed25519Config<()>> = args
        .value_of("config")
        .map(|f| {
            let mut f = File::open(f).unwrap();
            let mut s = String::new();
            f.read_to_string(&mut s).unwrap();
            toml::from_str(&s).unwrap()
        })
        .unwrap_or_default();

    if config.udp.disable {
        log::error!("udp tracker disabled");
        std::process::exit(1);
    }

    let tracker = Arc::new(Tracker::with_extension(
        Ed25519::new(config.extensions),
        config.tracker,
    ));

    let tracker_clone = tracker.clone();
    tokio::spawn(async move {
        tracker_clone.run_clean_loop().await;
    });

    let mut udp_join_handle = if config.udp.disable {
        tokio::spawn(async {})
    } else {
        match UdpTracker::bind(tracker, config.udp).await {
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
