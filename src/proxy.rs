use bytes::Bytes;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;

use crate::router::Route;

pub async fn forward_request(
    req: Request<hyper::body::Incoming>,
    route: &Route,
    client_ip: std::net::IpAddr,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    println!("--> {} {} -> {}", method, path, route.backend_url);

    let backend_stream =
        tokio::net::TcpStream::connect(format!("{}:{}", route.backend_host, route.backend_port))
            .await;

    match backend_stream {
        Ok(stream) => {
            let io = TokioIo::new(stream);
            let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;

            tokio::task::spawn(async move {
                if let Err(err) = conn.await {
                    eprintln!("Connect error: {:?}", err);
                }
            });

            // Strip the route prefix from the path if needed
            let upstream_path = if path == route.path_prefix {
                "/".to_string()
            } else {
                path[route.path_prefix.len()..].to_string()
            };
            let upstream_path = if upstream_path.is_empty() || !upstream_path.starts_with('/') {
                format!("/{}", upstream_path)
            } else {
                upstream_path
            };

            let upstream_req = Request::builder()
                .method(method)
                .uri(&upstream_path)
                .header("Host", &route.backend_host)
                .header("X-Forwarded-For", client_ip.to_string())
                .body(req.into_body().boxed())
                .unwrap();

            let response = sender.send_request(upstream_req).await?;
            Ok(response.map(|b| b.boxed()))
        }
        Err(e) => {
            eprintln!("Failed to connect to backend {}:{}", route.backend_url, e);
            let body = Full::new(Bytes::from("502 Bad Gateway"))
                .map_err(|never| match never {})
                .boxed();
            Ok(Response::builder().status(502).body(body).unwrap())
        }
    }
}
