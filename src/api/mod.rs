//! The JSON API under `/api/v1`, for a future mobile app and integrations
//! such as Home Assistant. It calls the same queries and rules as the pages
//! and only turns their results into JSON.

mod handlers;

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use handlers::api_err;

use crate::web::{AppState, St};

/// With `VORRAT_API_TOKEN` set, `/api/` needs that bearer token. The pages don't
/// use the API, so they keep working without it.
pub(crate) async fn api_auth(State(s): St, req: Request, next: Next) -> Response {
    if req.uri().path().starts_with("/api/") && !s.cfg.api_token.is_empty() {
        let given = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or("");
        if !ct_eq(given.as_bytes(), s.cfg.api_token.as_bytes()) {
            return api_err(StatusCode::UNAUTHORIZED, "missing or wrong bearer token");
        }
    }
    next.run(req).await
}

/// Constant-time comparison, so the token can't be guessed byte by byte.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The routes of the JSON API.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/items",
            get(handlers::api_items).post(handlers::api_create_item),
        )
        .route("/api/v1/items/{id}", get(handlers::api_item))
        .route("/api/v1/expiring", get(handlers::api_expiring))
        .route("/api/v1/plan", get(handlers::api_plan))
        .route("/api/v1/digest", get(handlers::api_digest))
        .route("/api/v1/summary", get(handlers::api_summary))
        .route("/api/v1/barcode/{code}", get(handlers::api_barcode))
        .route("/api/v1/items/{id}/batches", post(handlers::api_add_batch))
        .route("/api/v1/items/{id}/consume", post(handlers::api_consume))
        .route(
            "/api/v1/shopping",
            get(handlers::api_shopping).post(handlers::api_add_shopping),
        )
        .route("/api/v1/suggestions", get(handlers::api_suggestions))
}
