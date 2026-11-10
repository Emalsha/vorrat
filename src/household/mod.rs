//! The household: people by age group with their needs, pets, and the
//! household plan (meeting points, contacts, shut-offs) for when it is needed.

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
        .route(
            "/household",
            get(pages::household_page).post(pages::add_member),
        )
        .route(
            "/household/plan",
            get(pages::household_plan_page).post(pages::save_household_plan),
        )
        .route("/household/plan/print", get(pages::household_plan_print))
        .route("/household/{id}", post(pages::edit_member))
}
