use bytes::Bytes;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::{Request, Response};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};

use crate::router::Backend;

/// Keeps idle keep-alive connections to backends and reuses them.
pub type HttpClient = Client<HttpConnector, BoxBody<Bytes, hyper::Error>>;

pub fn new_client() -> HttpClient {
    Client::builder(TokioExecutor::new()).build_http()
}

pub async fn forward_request(
    client: &HttpClient,
    req: Request<BoxBody<Bytes, hyper::Error>>,
    backend: &Backend,
    client_ip: std::net::IpAddr,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    let (mut parts, body) = req.into_parts();

    // The pooled client needs an absolute URI
    // it still send only path + query upstream
    let path_and_query = parts.uri.path_and_query().map_or("/", |p| p.as_str());
    parts.uri = format!("http://{}{}", backend.address(), path_and_query)
        .parse()
        .unwrap();

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

    match client.request(Request::from_parts(parts, body)).await {
        Ok(res) => Ok(res.map(|b| b.boxed())),
        Err(e) => {
            eprintln!("Backend {} failed: {}", backend.address(), e);
            let body = Full::new(Bytes::from("502 Bad Gateway"))
                .map_err(|never| match never {})
                .boxed();
            Ok(Response::builder().status(502).body(body).unwrap())
        }
    }
}
