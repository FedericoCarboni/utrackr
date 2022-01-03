use std::{convert::Infallible, net::SocketAddr};

use hyper::{
    service::{make_service_fn, service_fn},
    server::conn::AddrStream,
    Body, Request, Response, Server,
};

async fn announce(addr: SocketAddr, req: Request<Body>) -> Result<Response<Body>, Infallible>  {
    let query = form_urlencoded::parse(req.uri().query().unwrap_or("").as_bytes());
    let info_hash = 0u32;
    for (key, value) in query {
        match &*key {
            "info_hash" => {

            },
            _ => {}
        }
    }
    Ok(Response::new((addr.to_string() + req.uri().path()).into()))
}

#[tokio::main]
async fn main() {
    // A `Service` is needed for every connection, so this
    // creates one from our `hello_world` function.
    let make_service = make_service_fn(|conn: &AddrStream| {
        let addr = conn.remote_addr();
        // service_fn converts our function into a `Service`
        let service = service_fn(move |req| {
            announce(addr, req)
        });

        // Return the service to hyper.
        async move { Ok::<_, Infallible>(service) }
    });

    // We'll bind to 127.0.0.1:3000
    let addr = SocketAddr::from(([0, 0, 0, 0], 9720));
    let server = Server::bind(&addr).serve::<_, Body>(make_service);

    // Run this server for... forever!
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}
