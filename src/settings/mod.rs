//! Settings: language, test push, backup, export and import.

mod pages;

use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;

use crate::web::AppState;

/// The routes of this feature.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/settings", get(pages::settings_page))
        .route("/settings/test-push", post(pages::test_push))
        .route("/settings/backup", post(pages::backup_now))
        .route("/settings/lang", post(pages::set_lang))
        .route("/export/vorrat.json", get(pages::export_json))
        .route("/export/stock.csv", get(pages::export_csv))
        .route("/export/vorrat.db", get(pages::export_db))
        .route(
            "/import",
            post(pages::import_csv).layer(DefaultBodyLimit::max(pages::MAX_IMPORT + 64 * 1024)),
        )
}
