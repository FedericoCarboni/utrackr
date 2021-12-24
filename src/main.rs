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

    // let client = Client::open("redis://127.0.0.1/")?;

    // let mut conn = client.get_async_connection().await?;
    // conn.set(b"helllo", b"helllo").await?;

    // let v: String = conn.get(b"helllo").await?;

    // println!("{:?}", v);

    let tracker = UdpTracker::bind("127.0.0.1:2710").await?;

    tracker.run().await?;

    Ok(())
}
