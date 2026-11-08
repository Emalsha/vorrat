//! The emergency plan: targets, coverage, the printable checklist and the power-cut view.

pub(crate) mod pages;
pub mod queries;
pub mod rules;

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;

use crate::web::AppState;

/// The routes of this feature.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/plan", get(pages::plan_page))
        .route("/plan/print", get(pages::plan_print))
        .route("/plan/power-cut", get(pages::power_cut_page))
        .route("/plan/settings", post(pages::plan_settings))
        .route("/plan/targets", post(pages::add_target))
        .route("/plan/targets/{id}", post(pages::edit_target))
        .route("/plan/to-shopping", post(pages::plan_to_shopping))
}
