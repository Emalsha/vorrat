//! The home page: what needs attention today.

mod pages;

use std::sync::Arc;

use axum::routing::get;
use axum::Router;

use crate::web::AppState;

/// The routes of this feature.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(pages::home))
}
