use std::sync::Arc;

use tokio::sync::RwLock;

use utrackr_core::Tracker;
use utrackr_http::announce::announce;

#[tokio::main]
async fn main() {
    let tracker = Tracker::default();
    warp::serve(announce(Arc::new(RwLock::new(tracker))))
        .run(([0, 0, 0, 0], 6969))
        .await;
}
