use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::env;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = env::args()
        .nth(1)
        .unwrap_or_else(|| "8080".to_string())
        .parse()
        .expect("Invalid port number");

    let name = env::args()
        .nth(2)
        .unwrap_or_else(|| format!("backend-{}", port));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    println!("{} listening on http://{}", name, addr);

    loop {
        let (stream, peer) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let name = name.clone();
        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(move |req| {
                        let name = name.clone();
                        async move { handle(req, &name, peer.ip()).await }
                    }),
                )
                .await
            {
                eprintln!("Error: {:?}", err);
            }
        });
    }
}

async fn handle(
    _req: Request<hyper::body::Incoming>,
    name: &str,
    _client_ip: std::net::IpAddr,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    // 100-200 ms so requests are always in flight when the version switches
    let jitter = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos()
        % 101;
    tokio::time::sleep(std::time::Duration::from_millis(100 + jitter as u64)).await;
    Ok(Response::new(Full::new(Bytes::from(format!("{name}\n")))))
}
