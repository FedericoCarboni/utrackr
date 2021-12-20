use std::io;

use utrackr_core::Tracker;

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
        .chain(fern::log_file("output.log")?)
        .apply()?;
    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    setup_logger().unwrap();

    let tracker = Tracker::bind("127.0.0.1:9000").await?;

    tracker.run().await?;

    Ok(())
}
