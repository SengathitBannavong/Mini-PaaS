use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bollard::Docker;
use bollard::plugin::{ContainerSummary, ContainerSummaryStateEnum};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::control::db;
use crate::control::docker::{self, Error, Replica};
use crate::router::{Backend, Route, Router, health};

/// Everything the control plane shares between request
pub struct AppState {
    pub router: Arc<Router>,
    pub docker: Docker,
    /// one Deployment at a time across all Apps
    pub deploy_lock: tokio::sync::Mutex<()>,
    pub api_key: String,
    /// std Mutex: lock only for one query, never across an .await
    pub db: Mutex<Connection>,
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
    /// How long each step took, to explain the Route-effective time
    pub pull_ms: u128,
    pub start_ms: u128,
    pub ready_ms: u128,
    pub drain_ms: u128,
    /// In-flight requests on blue still running when the 30s drain limit hit
    pub cut: usize,
}

/// One Deployment, recorded in SQLite from start to end
pub async fn deploy(s: Arc<AppState>, host: String, req: DeployReq) -> Result<DeployRes, Error> {
    // global lock, per-host locks if parallel deploys matter
    let _guard = s.deploy_lock.lock().await;
    let id = db::start(
        &s.db.lock().unwrap(),
        &host,
        &req.image,
        req.replicas.unwrap_or(1),
    )?;
    let res = run(&s, id, &host, req).await;
    let err = res.as_ref().err().map(|e| e.to_string());
    if let Err(e) = db::finish(&s.db.lock().unwrap(), id, err.as_deref()) {
        eprintln!("  [deploy] record finish of #{id}: {e}");
    }
    res
}

/// pull -> start green -> wait Ready -> swap Route -> drain blue (<= 30s) -> remove blue
async fn run(s: &AppState, id: i64, host: &str, req: DeployReq) -> Result<DeployRes, Error> {
    let host = host.to_string();
    let health_path = req.health_path.unwrap_or_else(|| "/".to_string());
    let n = req.replicas.unwrap_or(1).max(1);
    let t = Instant::now();

    // blue = what runs now (listed before green exists)
    let blue: Vec<String> = docker::list(&s.docker, Some(&host))
        .await?
        .into_iter()
        .filter_map(|c| c.id)
        .collect();

    let spec = docker::pull(&s.docker, &req.image, req.port).await?;
    db::set_digest(&s.db.lock().unwrap(), id, &spec.digest)?;
    let pull_ms = t.elapsed().as_millis();
    let t = Instant::now();

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

    let start_ms = t.elapsed().as_millis();
    let t = Instant::now();

    let urls: Vec<&str> = green.iter().map(|r| r.url.as_str()).collect();
    if !wait_ready(&urls, &health_path).await {
        remove_all(&s.docker, green.iter().map(|r| r.id.as_str())).await;
        return Err(format!("not Ready within 60s: GET {health_path} on {urls:?}").into());
    }
    let ready_ms = t.elapsed().as_millis();

    // blue's Route as requests see it now: its Replicas carry the In-flight counters
    let old_routes = s.router.find_route(&host);
    s.router.set_route(&host, &health_path, urls.clone()); // Swap here
    let t = Instant::now();
    let cut = match old_routes {
        Some(old) => drain(&old, Duration::from_secs(30)).await,
        None => 0,
    };
    let drain_ms = t.elapsed().as_millis();

    remove_all(&s.docker, blue.iter().map(String::as_str)).await;
    println!(
        "  [deploy] {host}: pull {pull_ms} ms, start {start_ms} ms, ready {ready_ms} ms, drain {drain_ms} ms, cut {cut}"
    );

    Ok(DeployRes {
        digest: spec.digest,
        replicas: urls.iter().map(|u| u.to_string()).collect(),
        pull_ms,
        start_ms,
        ready_ms,
        drain_ms,
        cut,
    })
}

/// Draining: wait until blue has no in-flight requests, at most `limit`
/// Returns how many were still running at the limit (they are cut and count as Failed requests).
async fn drain(old: &Route, limit: Duration) -> usize {
    let deadline = Instant::now() + limit;
    loop {
        // Sleep first: a request that picked blue just before the swap gets time to register
        // 50ms polling
        tokio::time::sleep(Duration::from_millis(50)).await;
        let n = old.in_flight();
        if n == 0 || Instant::now() >= deadline {
            return n;
        }
    }
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

/// Rebuild the routing table from running containers
/// Per host: keep the Version of the newest running container, remove everything else.
pub async fn recover(s: &AppState) -> Result<(), Error> {
    let mut by_host: HashMap<String, Vec<ContainerSummary>> = HashMap::new();

    for c in docker::list(&s.docker, None).await? {
        if let Some(host) = label(&c, "minipaas.host") {
            by_host.entry(host).or_default().push(c);
        }
    }

    for (host, mut cs) in by_host {
        cs.sort_by_key(|c| std::cmp::Reverse(c.created));
        let running = |c: &ContainerSummary| c.state == Some(ContainerSummaryStateEnum::RUNNING);

        // image_id is the same for every container of one digest
        let version = cs
            .iter()
            .find(|c| running(c))
            .and_then(|c| c.image_id.clone());
        let (keep, old): (Vec<_>, Vec<_>) = cs
            .into_iter()
            .partition(|c| running(c) && c.image_id == version);

        remove_all(&s.docker, old.iter().filter_map(|c| c.id.as_deref())).await;

        let mut urls = Vec::new();
        for c in &keep {
            let port = label(c, "minipaas.port").and_then(|p| p.parse().ok());
            if let (Some(id), Some(port)) = (c.id.as_deref(), port) {
                urls.extend(docker::url_of(&s.docker, id, port).await);
            }
        }

        if let Some(first) = keep.first()
            && !urls.is_empty()
        {
            let path = label(first, "minipaas.health_path").unwrap_or_else(|| "/".to_string());
            s.router
                .set_route(&host, &path, urls.iter().map(String::as_str).collect());
        }
    }

    Ok(())
}

fn label(c: &ContainerSummary, key: &str) -> Option<String> {
    c.labels.as_ref()?.get(key).cloned()
}

async fn remove_all(docker: &Docker, ids: impl Iterator<Item = &str>) {
    for id in ids {
        if let Err(e) = docker::remove(docker, id).await {
            eprintln!("  [deploy] remove {id}: {e}");
        }
    }
}
