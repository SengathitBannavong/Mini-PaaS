use bytes::Bytes;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;

use crate::router::Backend;

pub async fn forward_request(
    req: Request<BoxBody<Bytes, hyper::Error>>,
    backend: &Backend,
    client_ip: std::net::IpAddr,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    let path = req.uri().path().to_string();
    let method = req.method().clone();
    let url = backend.address();
    println!("--> {} {} -> {}", method, path, url);

    let backend_stream = tokio::net::TcpStream::connect(url.clone()).await;

    match backend_stream {
        Ok(stream) => {
            let io = TokioIo::new(stream);
            let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;

            tokio::task::spawn(async move {
                if let Err(err) = conn.await {
                    eprintln!("Connect error: {:?}", err);
                }
            });

            let (mut parts, body) = req.into_parts();

            // header not forward
            for h in [
                "connection",
                "keep-alive",
                "proxy-connection",
                "te",
                "trailer",
                "transfer-encoding",
                "upgrade",
            ] {
                parts.headers.remove(h);
            }
            parts
                .headers
                .insert("x-forwarded-for", client_ip.to_string().parse().unwrap());

            let upstream_req = Request::from_parts(parts, body);

            let response = sender.send_request(upstream_req).await?;
            Ok(response.map(|b| b.boxed()))
        }
        Err(e) => {
            eprintln!("Failed to connect to backend {}:{}", url, e);
            let body = Full::new(Bytes::from("502 Bad Gateway"))
                .map_err(|never| match never {})
                .boxed();
            Ok(Response::builder().status(502).body(body).unwrap())
        }
    }
}
