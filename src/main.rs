mod middleware;
mod proxy;
mod ratelimit;
mod router;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use middleware::ratelimit::RateLimitMiddleware;
use ratelimit::{RateLimitConfig, RateLimiter};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::middleware::cors::CorsMiddleware;
use crate::middleware::headers::AddHeadersMiddleware;
use crate::middleware::logging::LoggingMiddleware;
use crate::middleware::{MiddlewarePipeline, RequestContext};
use crate::router::Router;

struct Gateway {
    router: Router,
    pipeline: MiddlewarePipeline,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure our routes
    let mut router = Router::new();
    println!("Configuring routes...");
    router.add_route("/", "http://127.0.0.1:8080");
    router.add_route("/api/users", "http://127.0.0.1:8081");
    router.add_route("/api/orders", "http://127.0.0.1:8082");

    // Configure middleware
    let rate_config = RateLimitConfig::new(10, 1.0);
    let rate_limiter = RateLimiter::new(rate_config);
    let mut pipeline = MiddlewarePipeline::new();
    println!("Configuring middleware...");
    pipeline.add(Box::new(LoggingMiddleware));
    pipeline.add(Box::new(RateLimitMiddleware::new(rate_limiter)));
    let allowed_origins = vec!["http://a.com".to_string()];
    pipeline.add(Box::new(CorsMiddleware::new(allowed_origins)));
    pipeline.add(Box::new(
        AddHeadersMiddleware::new()
            .add_request_header("X-Gateway", "reverse-proxy")
            .add_response_header("X-Powered-By", "ReverseProxy/0.1"),
    ));

    let gateway = Arc::new(Gateway { router, pipeline });

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;

    println!("Reverse proxy API Gateway listening on http://{}", addr);

    loop {
        let (stream, peer) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let gateway = gateway.clone();

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(move |req| {
                        let gateway = gateway.clone();
                        async move { handle_request(req, &gateway, peer.ip()).await }
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
    gateway: &Gateway,
    client_ip: std::net::IpAddr,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    let ctx = RequestContext {
        client_ip: client_ip.to_string(),
        start_time: std::time::Instant::now(),
        origin: req
            .headers()
            .get("Origin")
            .and_then(|v| v.to_str().ok())
            .map(String::from),
    };

    let path = req.uri().path().to_string();

    // Convert the incomming body to a boxed body for middleware
    let (parts, body) = req.into_parts();
    let boxed_req = Request::from_parts(parts, body.boxed());

    // Run request middleware
    let boxed_req = match gateway.pipeline.process_request(boxed_req, &ctx) {
        Ok(req) => req,
        Err(response) => return Ok(gateway.pipeline.process_response(response, &ctx)),
    };

    // Route and proxy
    let response = match gateway.router.find_route(&path) {
        Some(route) => proxy::forward_request(boxed_req, route, client_ip).await?,
        None => {
            let body = Full::new(Bytes::from("404 Not Found"))
                .map_err(|never| match never {})
                .boxed();
            Response::builder().status(404).body(body).unwrap()
        }
    };

    Ok(gateway.pipeline.process_response(response, &ctx))
}
