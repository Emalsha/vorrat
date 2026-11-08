//! Responses shared by every page: errors, redirects with a message, downloads.

use askama::Template;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;

pub struct AppError(pub(crate) anyhow::Error);

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("{:#}", self.0);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Something went wrong. Details are in the server log.",
        )
            .into_response()
    }
}

pub(crate) type Res<T> = Result<T, AppError>;

pub(crate) fn render(t: impl Template) -> Res<Html<String>> {
    Ok(Html(t.render()?))
}

pub(crate) fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not found").into_response()
}

/// Back to a page with a short message ("Added 2 × Rice").
pub(crate) fn back(to: &str, msg: impl AsRef<str>) -> Response {
    let sep = if to.contains('?') { '&' } else { '?' };
    Redirect::to(&format!("{to}{sep}msg={}", url_encode(msg.as_ref()))).into_response()
}

/// After a form action: the success message, or the validation error.
pub(crate) fn done(to: &str, r: anyhow::Result<String>) -> Response {
    match r {
        Ok(m) => back(to, m),
        Err(e) => back(to, format!("Not saved: {e}")),
    }
}

pub(crate) fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The message and undo link a page shows after an action.
///
/// - `qty`: prefill for the add-stock form (from the shopping list).
#[derive(Default, Deserialize)]
pub struct Flash {
    pub(crate) msg: Option<String>,
    pub(crate) undo: Option<String>,
    pub(crate) qty: Option<String>,
}

/// Redirect target from a form: only our own pages (no open redirect).
pub(crate) fn safe_back(b: Option<String>, default: String) -> String {
    b.filter(|b| b.starts_with('/') && !b.starts_with("//") && !b.contains('\\'))
        .unwrap_or(default)
}

pub(crate) fn back_undo(to: &str, msg: impl AsRef<str>, token: &str) -> Response {
    let sep = if to.contains('?') { '&' } else { '?' };
    Redirect::to(&format!(
        "{to}{sep}msg={}&undo={}",
        url_encode(msg.as_ref()),
        url_encode(token)
    ))
    .into_response()
}

pub(crate) fn download(ctype: &'static str, name: &str, body: Vec<u8>) -> Response {
    (
        [
            (header::CONTENT_TYPE, ctype.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        body,
    )
        .into_response()
}
