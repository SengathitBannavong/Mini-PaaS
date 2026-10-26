use std::sync::Arc;
use std::time::Duration;

use bollard::Docker;
use serde::{Deserialize, Serialize};

use crate::control::docker::{self, Error, Replica};
use crate::router::{Backend, Router, health};

/// Everything the control plane shares between requests
pub struct AppState {
    pub router: Arc<Router>,
    pub docker: Docker,
    /// one Deployment at a time across all Apps
    pub deploy_lock: tokio::sync::Mutex<()>,
}

#[derive(Deserialize)]
pub struct DeployReq {
    pub image: String,
    pub port: Option<u16>,
    pub health_path: Option<String>,
    pub replicas: Option<u8>,
}

#[derive(Serialize)]
pub struct DeployRes {
    pub digest: String,
    pub replicas: Vec<String>,
}

/// pull -> start green -> wait Ready -> swap Route -> wait 5 s -> remove blue
pub async fn deploy(s: Arc<AppState>, host: String, req: DeployReq) -> Result<DeployRes, Error> {
    // global lock, per-host locks if parallel deploys matter
    let wait_s = 5;
    let _guard = s.deploy_lock.lock().await;
    let health_path = req.health_path.unwrap_or_else(|| "/".to_string());
    let n = req.replicas.unwrap_or(1).max(1);

    // blue = what runs now (listed before green exists)
    let blue: Vec<String> = docker::list(&s.docker, Some(&host))
        .await?
        .into_iter()
        .filter_map(|c| c.id)
        .collect();

    let spec = docker::pull(&s.docker, &req.image, req.port).await?;
    let mut green: Vec<Replica> = Vec::new();
    for _ in 0..n {
        match docker::start_replica(&s.docker, &host, &health_path, &spec).await {
            Ok(r) => green.push(r),
            Err(e) => {
                remove_all(&s.docker, green.iter().map(|r| r.id.as_str())).await;
                return Err(e);
            }
        }
    }

    let urls: Vec<&str> = green.iter().map(|r| r.url.as_str()).collect();
    if !wait_ready(&urls, &health_path).await {
        remove_all(&s.docker, green.iter().map(|r| r.id.as_str())).await;
        return Err(format!("not Ready within 60 s: GET {health_path} on {urls:?}").into());
    }
    s.router.set_route(&host, &health_path, urls.clone());

    if !blue.is_empty() {
        // TODO: fixed wait instead of draining; replaced by in-flight counter in M4
        tokio::time::sleep(Duration::from_secs(wait_s)).await;
    }
    remove_all(&s.docker, blue.iter().map(String::as_str)).await;

    Ok(DeployRes {
        digest: spec.digest,
        replicas: urls.iter().map(|u| u.to_string()).collect(),
    })
}

/// Ready = every replica answers `path` with 2xx; checked every 100 ms, give up after 60 s
async fn wait_ready(urls: &[&str], path: &str) -> bool {
    let backends: Vec<Backend> = urls.iter().map(|u| Backend::new(u)).collect();
    let all = async {
        let mut one_try = false;
        loop {
            let checks = backends
                .iter()
                .map(|b| health::check_backend(&b.host, b.port, path));
            if futures_util::future::join_all(checks)
                .await
                .into_iter()
                .all(|ok| ok)
            {
                return;
            }
            if !one_try {
                println!("deploy: {:?} not completed on first try", urls);
                one_try = true;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(60), all)
        .await
        .is_ok()
}

async fn remove_all(docker: &Docker, ids: impl Iterator<Item = &str>) {
    for id in ids {
        if let Err(e) = docker::remove(docker, id).await {
            eprintln!("  [deploy] remove {id}: {e}");
        }
    }
}
