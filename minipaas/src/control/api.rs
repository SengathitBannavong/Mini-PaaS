use std::sync::Arc;

use axum::extract::{Json, Path, Request, State};
use axum::http::{StatusCode, header::AUTHORIZATION};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use serde_json::{Value, json};

use crate::control::deploy::{self, AppState, DeployReq, DeployRes};

pub fn app(state: Arc<AppState>) -> axum::Router {
    axum::Router::new()
        .route("/apps", get(list_apps))
        .route("/apps/{host}/deploy", post(deploy_handler))
        .layer(middleware::from_fn_with_state(state.clone(), require_key))
        .layer(middleware::from_fn(log_request))
        .with_state(state)
}

/// Every request needs `Authorization: Bearer <MINIPAAS_API_KEY>`
async fn require_key(
    State(s): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let got = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    // constant-time: don't leak how many leading bytes matched
    let ok = got.len() == s.api_key.len()
        && got
            .bytes()
            .zip(s.api_key.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0;
    if ok {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Routing table as JSON: host, health path, replicas and whether each is Ready
async fn list_apps(State(s): State<Arc<AppState>>) -> Json<Value> {
    let apps: Vec<Value> = s
        .router
        .all_routes()
        .iter()
        .map(|r| {
            let replicas: Vec<Value> = r
                .get_backends()
                .iter()
                .map(|b| json!({ "url": b.url, "ready": b.healthy }))
                .collect();
            json!({ "host": r.host, "health_path": r.health_path, "replicas": replicas })
        })
        .collect();
    Json(Value::Array(apps))
}

/// Same format as the router's LoggingMiddleware, tagged [api]. Never logs headers (API key).
async fn log_request(req: Request, next: Next) -> Response {
    let (method, path) = (req.method().clone(), req.uri().path().to_string());
    let start = std::time::Instant::now();
    let now = || chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    println!("[{}] [api] --> {method} {path}", now());
    let res = next.run(req).await;
    println!(
        "[{}] [api] <-- {} {method} {path} ({:.2}ms)",
        now(),
        res.status(),
        start.elapsed().as_secs_f64() * 1000.0
    );
    res
}

async fn deploy_handler(
    State(s): State<Arc<AppState>>,
    Path(host): Path<String>,
    Json(req): Json<DeployReq>,
) -> Result<Json<DeployRes>, (StatusCode, String)> {
    let host = host.to_ascii_lowercase();
    // host goes into a Docker label filter and the Routing table: keep it a plain domain name
    // host name allow only "a-z . -"
    if host.is_empty()
        || !host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return Err((StatusCode::BAD_REQUEST, format!("bad host {host:?}")));
    }
    // spawn: hyper drops the handler future on client disconnect; a deploy must not stop half-way
    tokio::spawn(deploy::deploy(s, host, req))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}
