//! The household: people by age group with their needs, and pets.

mod pages;
pub mod queries;

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;

use crate::web::AppState;

/// The routes of this feature.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/household",
            get(pages::household_page).post(pages::add_member),
        )
        .route("/household/{id}", post(pages::edit_member))
}
