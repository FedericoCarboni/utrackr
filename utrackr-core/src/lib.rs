use std::io;

mod protocol;
use protocol::UdpTracker;

pub struct Tracker {
    udp: UdpTracker,
}

impl Tracker {
    pub async fn run(&'static self) -> io::Result<()> {
        self.udp.run_forever().await
    }
}
