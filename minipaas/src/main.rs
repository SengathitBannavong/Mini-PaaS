// mod control; // TODO: enable once control/docker.rs exists
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

use crate::router::middleware::headers::AddHeadersMiddleware;
use crate::router::middleware::logging::LoggingMiddleware;
use crate::router::middleware::{MiddlewarePipeline, RequestContext};
use crate::router::{Router, health, proxy};

struct Gateway {
    router: Arc<Router>,
    pipeline: MiddlewarePipeline,
    client: proxy::HttpClient,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure our routes
    let router = Arc::new(Router::new());
    println!("Configuring routes...");
    router.set_route("app.localhost", vec!["http://127.0.0.1:18081"]);
    // router.set_route(
    //     "orders.localhost",
    //     vec!["http://127.0.0.1:8082", "http://127.0.0.1:8084"],
    // );

    // Start background health checks
    println!("Starting health checks...");
    health::start_health_checks(router.clone(), 10);

    // Configure middleware
    let mut pipeline = MiddlewarePipeline::new();
    println!("Configuring middleware...");
    pipeline.add(Box::new(LoggingMiddleware));
    pipeline.add(Box::new(
        AddHeadersMiddleware::new()
            .add_request_header("X-Gateway", "reverse-proxy")
            .add_response_header("X-Powered-By", "ReverseProxy/0.1"),
    ));

    let gateway = Arc::new(Gateway {
        router,
        pipeline,
        client: proxy::new_client(),
    });

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;

    println!("Reverse proxy API Gateway listening on http://{}", addr);

    serve(listener, gateway).await?;
    Ok(())
}

async fn serve(listener: TcpListener, gateway: Arc<Gateway>) -> std::io::Result<()> {
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
        _origin: req
            .headers()
            .get("Origin")
            .and_then(|v| v.to_str().ok())
            .map(String::from),
    };

    let host = req
        .headers()
        .get(hyper::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    // Convert the incomming body to a boxed body for middleware
    let (parts, body) = req.into_parts();
    let boxed_req = Request::from_parts(parts, body.boxed());

    // Run request middleware
    let boxed_req = match gateway.pipeline.process_request(boxed_req, &ctx) {
        Ok(req) => req,
        Err(response) => return Ok(gateway.pipeline.process_response(response, &ctx)),
    };

    // Route and proxy
    let response = match gateway.router.find_route(&host) {
        Some(route) => match route.next_backend() {
            Some(backend) => {
                proxy::forward_request(&gateway.client, boxed_req, &backend, client_ip).await?
            }
            None => {
                // All backends are down
                let body = Full::new(Bytes::from(
                    "503 Service Unavailable\nAll backends are unhealthy.\n",
                ))
                .map_err(|never| match never {})
                .boxed();
                Response::builder().status(503).body(body).unwrap()
            }
        },
        None => {
            let body = Full::new(Bytes::from("404 Not Found"))
                .map_err(|never| match never {})
                .boxed();
            Response::builder().status(404).body(body).unwrap()
        }
    };

    Ok(gateway.pipeline.process_response(response, &ctx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::Empty;
    use tokio::net::TcpStream;

    /// Backend that answers "<name> <uri> <cookie>" so the test can see what reached it.
    async fn spawn_backend(name: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                tokio::spawn(http1::Builder::new().serve_connection(
                    TokioIo::new(stream),
                    service_fn(move |req: Request<hyper::body::Incoming>| async move {
                        let cookie = req
                            .headers()
                            .get("cookie")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("")
                            .to_string();
                        let body = format!("{name} {} {cookie}", req.uri());
                        Ok::<_, hyper::Error>(Response::new(Full::new(Bytes::from(body))))
                    }),
                ));
            }
        });
        format!("http://{addr}")
    }

    /// Send GET `path` with `Host: host` and a cookie through the gateway.
    async fn get(gateway: SocketAddr, host: &str, path: &str) -> (u16, String) {
        let stream = TcpStream::connect(gateway).await.unwrap();
        let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .unwrap();
        tokio::spawn(conn);
        let req = Request::get(path)
            .header(hyper::header::HOST, host)
            .header("cookie", "a=1")
            .body(Empty::<Bytes>::new())
            .unwrap();
        let res = sender.send_request(req).await.unwrap();
        let status = res.status().as_u16();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn routes_by_host_and_swaps_at_runtime() {
        let blue = spawn_backend("blue").await;
        let green = spawn_backend("green").await;

        let router = Arc::new(Router::new());
        router.set_route("app.localhost", vec![blue.as_str()]);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let gateway = Gateway {
            router: router.clone(),
            pipeline: MiddlewarePipeline::new(),
            client: proxy::new_client(),
        };
        tokio::spawn(serve(listener, Arc::new(gateway)));

        // Host is matched ignoring case and port; path, query and cookie reach the backend untouched
        assert_eq!(
            get(addr, "App.Localhost:3000", "/api/users?q=1").await,
            (200, "blue /api/users?q=1 a=1".to_string())
        );

        // Unknown host
        assert_eq!(get(addr, "nope.localhost", "/").await.0, 404);

        // Swap while running: the next request goes to green
        router.set_route("app.localhost", vec![green.as_str()]);
        assert_eq!(get(addr, "app.localhost", "/").await.1, "green / a=1");
    }
}
