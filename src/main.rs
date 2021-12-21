use std::io;

use utrackr_core::UdpTracker;

fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}] {}",
                record.target(),
                record.level(),
                message
            ))
        })
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    setup_logger().unwrap();

    let tracker = UdpTracker::bind("127.0.0.1:2710").await?;

    tracker.run().await?;

    Ok(())
}
