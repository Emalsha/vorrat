//! The web plumbing every feature shares: app state, the router that merges
//! each feature's routes, the language and security middleware, and static
//! files. Pages live in their feature folders.

pub mod form;
pub mod respond;
pub mod view;

use std::sync::Arc;

use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::auth::Sessions;
use crate::config::Config;
use crate::db::Store;
use crate::jobs::notify::Notifier;
use crate::web::respond::{back, not_found};
use crate::web::view::DEMO;
use crate::{
    api, auth, country, essentials, grab_bag, home, household, jobs, locations, plan, settings,
    shopping, stock,
};

/// Shared state of the running app.
///
/// - `lang`: the saved language setting (`auto`, `en`, `de`), kept here so a page
///   view doesn't query it.
pub struct AppState {
    pub store: Store,
    pub cfg: Config,
    pub notifier: Option<Notifier>,
    pub(crate) lang: std::sync::RwLock<String>,
    pub sessions: Sessions,
}

impl AppState {
    pub fn new(store: Store, cfg: Config) -> Self {
        let notifier = jobs::notify::ntfy(&cfg);
        let lang = store
            .setting("lang")
            .ok()
            .flatten()
            .unwrap_or_else(|| "auto".into());
        Self {
            store,
            cfg,
            notifier,
            lang: std::sync::RwLock::new(lang),
            sessions: Default::default(),
        }
    }

    pub fn lang_setting(&self) -> String {
        self.lang.read().map(|l| l.clone()).unwrap_or_default()
    }
}

pub(crate) type St = State<Arc<AppState>>;

/// All pages and the API behind the same layers: the API token check, the
/// household password, the request language and the security headers. Each
/// feature brings its own routes, so a new page can't miss a layer.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(home::routes())
        .merge(stock::routes())
        .merge(locations::routes())
        .merge(plan::routes())
        .merge(household::routes())
        .merge(country::routes())
        .merge(grab_bag::routes())
        .merge(essentials::routes())
        .merge(shopping::routes())
        .merge(settings::routes())
        .route("/files/{name}", get(data_file))
        .route("/static/{name}", get(static_file))
        .route("/sw.js", get(service_worker))
        .route("/healthz", get(|| async { "ok" }))
        .merge(api::routes())
        .layer(middleware::from_fn_with_state(state.clone(), api::api_auth))
        .merge(auth::routes())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::pages::sign_in,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), language))
        .layer(middleware::from_fn(security))
        .with_state(state)
}

async fn static_file(Path(name): Path<String>) -> Response {
    let (body, ctype): (&'static [u8], &str) = match name.as_str() {
        "app.css" => (
            include_bytes!("../../static/app.css"),
            "text/css; charset=utf-8",
        ),
        "manifest.webmanifest" => (
            include_bytes!("../../static/manifest.webmanifest"),
            "application/manifest+json",
        ),
        "app.js" => (
            include_bytes!("../../static/app.js"),
            "text/javascript; charset=utf-8",
        ),
        "print.js" => (
            include_bytes!("../../static/print.js"),
            "text/javascript; charset=utf-8",
        ),
        "scan.js" => (
            include_bytes!("../../static/scan.js"),
            "text/javascript; charset=utf-8",
        ),
        "zxing-0.23.0.min.js" => (
            include_bytes!("../../static/vendor/zxing-0.23.0.min.js"),
            "text/javascript; charset=utf-8",
        ),
        "jost.woff2" => (
            include_bytes!("../../static/fonts/jost.woff2"),
            "font/woff2",
        ),
        "icon-180.png" => (include_bytes!("../../static/icon-180.png"), "image/png"),
        "icon-192.png" => (include_bytes!("../../static/icon-192.png"), "image/png"),
        "icon-512.png" => (include_bytes!("../../static/icon-512.png"), "image/png"),
        _ => return not_found(),
    };
    (
        [
            (header::CONTENT_TYPE, ctype),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        body,
    )
        .into_response()
}

/// Served from the root so it may control every page. The version in the cache
/// name makes an upgrade replace the saved copies.
async fn service_worker() -> Response {
    static SW: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let body = SW.get_or_init(|| {
        include_str!("../../static/sw.js").replace("__VERSION__", env!("CARGO_PKG_VERSION"))
    });
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body.as_str(),
    )
        .into_response()
}

async fn data_file(State(s): St, Path(name): Path<String>) -> Response {
    if !crate::files::is_safe_name(&name) {
        return not_found();
    }
    match tokio::fs::read(s.cfg.files_dir().join(&name)).await {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, crate::files::content_type(&name)),
                (header::CACHE_CONTROL, "private, max-age=604800, immutable"),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => not_found(),
    }
}

/// On a demo instance, nothing that stores files or changes the server.
fn demo_blocks(method: &Method, path: &str) -> bool {
    method == Method::POST
        && (path.ends_with("/attachments") || path == "/import" || path.starts_with("/settings/"))
}

/// Every request runs with its language: the saved setting, else the browser's.
async fn language(State(s): St, req: Request, next: Next) -> Response {
    let lang = crate::i18n::Lang::pick(
        &s.lang_setting(),
        req.headers()
            .get(header::ACCEPT_LANGUAGE)
            .and_then(|v| v.to_str().ok()),
    );
    if s.cfg.demo && demo_blocks(req.method(), req.uri().path()) {
        let msg =
            crate::i18n::LANG.scope(lang, async { crate::i18n::tr("Not available in the demo") });
        return back("/settings", msg.await);
    }
    let demo = s.cfg.demo;
    crate::i18n::LANG
        .scope(lang, DEMO.scope(demo, next.run(req)))
        .await
}

/// Strict headers on every response, and a same-origin check on every write:
/// a page on another site can't make the browser post to Vorrat (CSRF).
/// A handler may set a stricter policy of its own (attachments: sandbox).
async fn security(req: Request, next: Next) -> Response {
    if req.method() != Method::GET && req.method() != Method::HEAD && !same_origin(req.headers()) {
        return (StatusCode::FORBIDDEN, "cross-site request refused").into_response();
    }
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    let set = |h: &mut HeaderMap, k: &'static str, v: &'static str| {
        h.insert(k, HeaderValue::from_static(v));
    };
    if !h.contains_key("content-security-policy") {
        set(
            h,
            "content-security-policy",
            "default-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self'; \
         object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
        );
    }
    set(h, "x-content-type-options", "nosniff");
    set(h, "referrer-policy", "no-referrer");
    set(h, "x-frame-options", "DENY");
    set(
        h,
        "permissions-policy",
        "camera=(self), geolocation=(), microphone=()",
    );
    res
}

/// Whether a write comes from Vorrat's own pages, by `Sec-Fetch-Site`. Older
/// browsers: the host in `Origin` must match `Host`. No `Origin` means it is not
/// a browser form.
fn same_origin(h: &HeaderMap) -> bool {
    if let Some(site) = h.get("sec-fetch-site").and_then(|v| v.to_str().ok()) {
        return matches!(site, "same-origin" | "none");
    }
    match (
        h.get(header::ORIGIN).and_then(|v| v.to_str().ok()),
        h.get(header::HOST).and_then(|v| v.to_str().ok()),
    ) {
        (None, _) => true,
        (Some(origin), Some(host)) => origin.split("://").nth(1) == Some(host),
        (Some(_), None) => false,
    }
}
