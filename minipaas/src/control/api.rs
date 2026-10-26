use std::sync::Arc;

use axum::extract::{Json, Path, State};
use axum::http::StatusCode;
use axum::routing::post;

use crate::control::deploy::{self, AppState, DeployReq, DeployRes};

pub fn app(state: Arc<AppState>) -> axum::Router {
    axum::Router::new()
        .route("/apps/{host}/deploy", post(deploy_handler))
        .with_state(state)
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
