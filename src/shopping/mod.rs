//! The shopping list, with learned suggestions.

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
            "/shopping",
            get(pages::shopping_page).post(pages::add_shopping),
        )
        .route("/shopping/{id}/toggle", post(pages::toggle_shopping))
        .route("/shopping/{id}/stock", post(pages::shopping_to_stock))
        .route("/shopping/add-low", post(pages::shopping_add_low))
        .route(
            "/shopping/suggestions/{id}/dismiss",
            post(pages::dismiss_suggestion),
        )
        .route("/shopping/clear", post(pages::clear_shopping))
}
