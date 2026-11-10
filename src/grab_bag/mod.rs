//! Grab bags: a bag per person for leaving home quickly, with its checklist.

mod pages;
pub mod rules;

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;

use crate::web::AppState;

/// The routes of this feature.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/grab-bag", get(pages::grab_bag_page))
        .route("/grab-bag/{member}", post(pages::make_bag))
}
