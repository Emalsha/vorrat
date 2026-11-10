//! Country presets: the household picks its country and can use that
//! authority's plan length and drinking-water amount.

mod pages;
pub mod rules;

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;

use crate::web::AppState;

/// The routes of this feature.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/country", get(pages::country_page).post(pages::choose))
        .route("/country/apply", post(pages::apply))
}
