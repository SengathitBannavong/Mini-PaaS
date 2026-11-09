use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use hyper_util::server::graceful::GracefulShutdown;
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

    let graceful = GracefulShutdown::new();
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;

    loop {
        let (stream, peer) = tokio::select! {
            conn = listener.accept() => conn?,
                _ = term.recv() => break,
        };
        let io = TokioIo::new(stream);
        let name = name.clone();
        let conn = http1::Builder::new().serve_connection(
            io,
            service_fn(move |req| {
                let name = name.clone();
                async move { handle(req, &name, peer.ip()).await }
            }),
        );
        let conn = graceful.watch(conn);
        tokio::task::spawn(async move {
            if let Err(err) = conn.await {
                eprintln!("Error: {:?}", err);
            }
        });
    }
    drop(listener);
    graceful.shutdown().await;
    println!("{} stopped", name);
    Ok(())
}

async fn handle(
    req: Request<hyper::body::Incoming>,
    name: &str,
    _client_ip: std::net::IpAddr,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    // 100-200 ms so requests are always in flight when the version switches
    let jitter = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos()
        % 101;
    // ?delay=MS: a long request (upload, report) to see what each system does with it
    let ms = req
        .uri()
        .query()
        .and_then(|q| q.strip_prefix("delay="))
        .and_then(|v| v.parse().ok())
        .unwrap_or(100 + jitter as u64);
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    Ok(Response::new(Full::new(Bytes::from(format!("{name}\n")))))
}
