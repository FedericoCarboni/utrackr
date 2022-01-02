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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger().unwrap();

    let tracker = UdpTracker::bind("0.0.0.0:6270").await?;

    tracker.run().await?;

    Ok(())
}
