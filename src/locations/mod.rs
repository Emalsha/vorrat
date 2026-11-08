//! Places in the house where things are stored.

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
            "/locations",
            get(pages::locations_page).post(pages::add_location),
        )
        .route("/locations/{id}", post(pages::edit_location))
}
