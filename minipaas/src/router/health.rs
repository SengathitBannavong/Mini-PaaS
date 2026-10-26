use crate::router::Router;
use http_body_util::Empty;
use hyper::{Request, header::HOST};
use hyper_util::rt::TokioIo;
use std::{sync::Arc, time::Duration};
use tokio::{net::TcpStream, time};

pub fn start_health_checks(router: Arc<Router>, interval_secs: u64) {
    tokio::task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(interval_secs));
        println!("    Health checks started (every {}s)", interval_secs);
        loop {
            interval.tick().await;
            for route in router.all_routes() {
                for backend in route.get_backends() {
                    let healthy =
                        check_backend(&backend.host, backend.port, &route.health_path).await;
                    route.set_backend_health(&backend.url, healthy);
                }
            }
        }
    });
}

/// Healthy = answer GET <path> with 2xx within 3s
async fn check_backend(host: &str, port: u16, path: &str) -> bool {
    let probe = async {
        let stream = TcpStream::connect((host, port)).await.ok()?;
        let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .ok()?;
        tokio::spawn(conn);
        let req = Request::get(path)
            .header(HOST, format!("{host}:{port}"))
            .body(Empty::<hyper::body::Bytes>::new())
            .ok()?;
        let res = sender.send_request(req).await.ok()?;
        Some(res.status().is_success())
    };
    matches!(
        time::timeout(Duration::from_secs(3), probe).await,
        Ok(Some(true))
    )
}
