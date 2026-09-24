use bytes::Bytes;
use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;

    println!("Reverse proxy API Gateway listening on http://{}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(handle_request))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    let backend_url = "http://127.0.0.1:8080";
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    println!("--> {} {}", method, path);

    let uri = format!("{}{}", backend_url, path);

    let host = "127.0.0.1";
    let port = 8080;
    let stream = TcpListener::bind("0.0.0.0:0").await.ok();
    let backend_stream = tokio::net::TcpStream::connect(format!("{}:{}", host, port)).await;

    match backend_stream {
        Ok(stream) => {
            let io = TokioIo::new(stream);

            let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;

            // Spawn the connection handler
            tokio::task::spawn(async move {
                if let Err(err) = conn.await {
                    eprintln!("Connection error: {:?}", err);
                }
            });

            // Forward the request
            let upstream_req = Request::builder()
                .method(method)
                .uri(&path)
                .header("Host", host)
                .body(req.into_body().boxed())
                .unwrap();
            let response = sender.send_request(upstream_req).await?;

            // Map the response body
            Ok(response.map(|b| b.boxed()))
        }
        Err(_) => {
            // Backend is unreadable
            let body = Full::new(Bytes::from("502 Bad Gateway"))
                .map_err(|never| match never {})
                .boxed();
            Ok(Response::builder().status(502).body(body).unwrap())
        }
    }
}
