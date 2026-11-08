//! Medicine with daily doses, the first-aid kit and the household essentials checklists.

mod pages;
pub mod queries;
pub mod rules;

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;

use crate::web::AppState;

/// The routes of this feature.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/essentials", get(pages::essentials_page))
        .route("/essentials/medicine", post(pages::add_medicine))
        .route("/essentials/check/{key}", post(pages::check_action))
        .route("/items/{id}/dose", post(pages::set_dose))
        .route("/items/{id}/preset", post(pages::clear_preset))
}
