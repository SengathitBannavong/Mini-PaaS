use crate::router::Route;
use std::{sync::Arc, time::Duration};
use tokio::{net::TcpStream, time};

pub fn start_health_checks(routes: Vec<Arc<Route>>, interval_secs: u64) {
    tokio::task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(interval_secs));
        println!("    Health checks started (every {}s)", interval_secs);

        loop {
            interval.tick().await;
            check_all_backends(&routes).await;
        }
    });
}

async fn check_all_backends(routes: &[Arc<Route>]) {
    for route in routes {
        let backends = route.get_backends();
        for backend in &backends {
            let healthy = check_backend(&backend.host, backend.port).await;
            if !healthy {
                println!("[health] {} is down", backend.address());
            }
            route.set_backend_health(&backend.url, healthy);
        }
    }
}

async fn check_backend(host: &str, port: u16) -> bool {
    let addr = format!("{}:{}", host, port);
    match time::timeout(Duration::from_secs(3), TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => true,   // connected successfully
        Ok(Err(_)) => false, // connected refused
        Err(_) => false,     // Timeout
    }
}
