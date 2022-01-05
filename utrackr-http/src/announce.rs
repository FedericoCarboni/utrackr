use std::time::Duration;

use tower::ServiceBuilder;

pub fn announce() {
    ServiceBuilder::new()
        .rate_limit(6, Duration::from_secs(60))
        .service_fn(|| {

        })
}
