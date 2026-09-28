mod proxy;
mod router;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::router::Router;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure our routes
    let mut router = Router::new();
    println!("Configuring routes...");
    router.add_route("/", "http://127.0.0.1:8080");
    router.add_route("/api/users", "http://127.0.0.1:8081");
    router.add_route("/api/orders", "http://127.0.0.1:8082");

    let router = Arc::new(router);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;

    println!("Reverse proxy API Gateway listening on http://{}", addr);

    loop {
        let (stream, peer) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let router = router.clone();

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(move |req| {
                        let router = router.clone();
                        async move { handle_request(req, &router, peer.ip()).await }
                    }),
                )
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    router: &Router,
    client_ip: std::net::IpAddr,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    let path = req.uri().path().to_string();

    match router.find_route(&path) {
        Some(route) => proxy::forward_request(req, route, client_ip).await,
        None => {
            let body = Full::new(Bytes::from("502 Bad Gateway"))
                .map_err(|never| match never {})
                .boxed();
            Ok(Response::builder().status(502).body(body).unwrap())
        }
    }
}
