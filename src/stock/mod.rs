//! Stock: items, batches, barcodes, attachments and the expiry calendar.

mod attachments;
pub(crate) mod pages;
pub mod queries;
pub mod rules;
mod scan;

use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;

use crate::web::AppState;

/// The routes of this feature.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/items", get(pages::items_page))
        .route("/items/{id}", get(pages::item_page))
        .route("/items/{id}/batches", post(pages::add_batch))
        .route("/items/{id}/use", post(pages::use_item))
        .route("/items/{id}/minimum", post(pages::set_minimum))
        .route("/items/{id}/delete", post(pages::delete_item))
        .route("/batches/{id}/delete", post(pages::delete_batch))
        .route("/batches/{id}/open", post(pages::open_batch))
        .route("/undo/{token}", post(pages::undo))
        .route(
            "/items/{id}/edit",
            get(pages::edit_item_page).post(pages::edit_item),
        )
        .route(
            "/items/{id}/attachments",
            post(attachments::upload)
                .layer(DefaultBodyLimit::max(attachments::MAX_UPLOAD + 64 * 1024)),
        )
        .route("/attachments/{id}", get(attachments::get_attachment))
        .route(
            "/attachments/{id}/delete",
            post(attachments::delete_attachment),
        )
        .route("/barcodes/link", post(scan::link_barcode))
        .route("/items/{id}/barcodes/delete", post(scan::unlink_barcode))
        .route("/calendar.ics", get(pages::calendar))
        .route("/add", get(pages::add_page).post(pages::add_submit))
        .route("/scan", get(scan::scan_page))
        .route("/scan/code", get(scan::scan_code))
}
