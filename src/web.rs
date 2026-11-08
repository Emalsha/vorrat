//! HTTP layer: server-rendered pages (no JavaScript needed) plus a JSON API
//! under `/api/v1` for future native or third-party clients.

use std::sync::Arc;

use askama::Template;
use axum::extract::{Form, Path, Query, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::NaiveDate;
use serde::Deserialize;

use crate::config::Config;
use crate::db::{self, ItemQuery, NewItem, Sort, Store, CATEGORIES, KINDS, UNITS};
use crate::i18n::tr;
use crate::notify::{self, Notifier};
use crate::rules::{self, fmt_qty, Dated, Status};

pub struct AppState {
    pub store: Store,
    pub cfg: Config,
    pub notifier: Option<Notifier>,
    /// The saved language setting (`auto`, `en`, `de`), kept here so a page
    /// view doesn't query it.
    lang: std::sync::RwLock<String>,
    pub sessions: crate::auth::Sessions,
}

impl AppState {
    pub fn new(store: Store, cfg: Config) -> Self {
        let notifier = notify::ntfy(&cfg);
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

type St = State<Arc<AppState>>;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/items", get(items_page))
        .route("/items/{id}", get(item_page))
        .route("/items/{id}/batches", post(add_batch))
        .route("/items/{id}/use", post(use_item))
        .route("/items/{id}/minimum", post(set_minimum))
        .route("/items/{id}/delete", post(delete_item))
        .route("/batches/{id}/delete", post(delete_batch))
        .route("/batches/{id}/open", post(open_batch))
        .route("/undo/{token}", post(undo))
        .route("/items/{id}/edit", get(edit_item_page).post(edit_item))
        .route(
            "/items/{id}/attachments",
            post(upload).layer(axum::extract::DefaultBodyLimit::max(MAX_UPLOAD + 64 * 1024)),
        )
        .route("/attachments/{id}", get(get_attachment))
        .route("/attachments/{id}/delete", post(delete_attachment))
        .route("/barcodes/link", post(link_barcode))
        .route("/items/{id}/barcodes/delete", post(unlink_barcode))
        .route("/locations", get(locations_page).post(add_location))
        .route("/locations/{id}", post(edit_location))
        .route("/calendar.ics", get(calendar))
        .route("/add", get(add_page).post(add_submit))
        .route("/scan", get(scan_page))
        .route("/scan/code", get(scan_code))
        .route("/files/{name}", get(data_file))
        .route("/plan", get(plan_page))
        .route("/plan/print", get(plan_print))
        .route("/plan/power-cut", get(power_cut_page))
        .route("/plan/settings", post(plan_settings))
        .route("/household", get(household_page).post(add_member))
        .route("/household/{id}", post(edit_member))
        .route("/essentials", get(essentials_page))
        .route("/essentials/medicine", post(add_medicine))
        .route("/essentials/check/{key}", post(check_action))
        .route("/items/{id}/dose", post(set_dose))
        .route("/items/{id}/preset", post(clear_preset))
        .route("/plan/targets", post(add_target))
        .route("/plan/targets/{id}", post(edit_target))
        .route("/plan/to-shopping", post(plan_to_shopping))
        .route("/shopping", get(shopping_page).post(add_shopping))
        .route("/shopping/{id}/toggle", post(toggle_shopping))
        .route("/shopping/{id}/stock", post(shopping_to_stock))
        .route("/shopping/add-low", post(shopping_add_low))
        .route(
            "/shopping/suggestions/{id}/dismiss",
            post(dismiss_suggestion),
        )
        .route("/shopping/clear", post(clear_shopping))
        .route("/settings", get(settings_page))
        .route("/settings/test-push", post(test_push))
        .route("/settings/backup", post(backup_now))
        .route("/settings/lang", post(set_lang))
        .route("/export/vorrat.json", get(export_json))
        .route("/export/stock.csv", get(export_csv))
        .route("/export/vorrat.db", get(export_db))
        .route(
            "/import",
            post(import_csv).layer(axum::extract::DefaultBodyLimit::max(MAX_IMPORT + 64 * 1024)),
        )
        .route("/static/{name}", get(static_file))
        .route("/sw.js", get(service_worker))
        .route("/healthz", get(|| async { "ok" }))
        .route("/api/v1/items", get(api_items).post(api_create_item))
        .route("/api/v1/items/{id}", get(api_item))
        .route("/api/v1/expiring", get(api_expiring))
        .route("/api/v1/plan", get(api_plan))
        .route("/api/v1/digest", get(api_digest))
        .route("/api/v1/summary", get(api_summary))
        .route("/api/v1/barcode/{code}", get(api_barcode))
        .route("/api/v1/items/{id}/batches", post(api_add_batch))
        .route("/api/v1/items/{id}/consume", post(api_consume))
        .route("/api/v1/shopping", get(api_shopping).post(api_add_shopping))
        .route("/api/v1/suggestions", get(api_suggestions))
        .layer(middleware::from_fn_with_state(state.clone(), api_auth))
        .route("/login", get(login_page).post(login))
        .route("/logout", post(logout))
        .layer(middleware::from_fn_with_state(state.clone(), sign_in))
        .layer(middleware::from_fn_with_state(state.clone(), language))
        .layer(middleware::from_fn(security))
        .with_state(state)
}

// -- language -------------------------------------------------------------------

tokio::task_local! {
    /// Whether this request is on a demo instance.
    static DEMO: bool;
}

/// Template filters: `{{ "Stock"|t }}`, and the demo banner text.
mod filters {
    pub fn t<T: std::fmt::Display>(s: T, _: &dyn askama::Values) -> askama::Result<String> {
        Ok(crate::i18n::tr(&s.to_string()))
    }

    pub fn lang_code<T: std::fmt::Display>(
        _: T,
        _: &dyn askama::Values,
    ) -> askama::Result<&'static str> {
        Ok(crate::i18n::current().code())
    }

    pub fn demo_note<T: std::fmt::Display>(_: T, _: &dyn askama::Values) -> askama::Result<String> {
        let on = super::DEMO.try_with(|d| *d).unwrap_or(false);
        Ok(if on {
            crate::i18n::tr("Demo: try anything. The data resets every night.")
        } else {
            String::new()
        })
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

// -- security -----------------------------------------------------------------

/// Strict headers on every response, and a same-origin check on every write:
/// a page on another site can't make the browser post to Vorrat (CSRF).
async fn security(req: Request, next: Next) -> Response {
    if req.method() != Method::GET && req.method() != Method::HEAD && !same_origin(req.headers()) {
        return (StatusCode::FORBIDDEN, "cross-site request refused").into_response();
    }
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    let set = |h: &mut HeaderMap, k: &'static str, v: &'static str| {
        h.insert(k, HeaderValue::from_static(v));
    };
    // A handler may set a stricter policy of its own (attachments: sandbox).
    if !h.contains_key("content-security-policy") {
        set(
        h,
        "content-security-policy",
        "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'; \
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

fn same_origin(h: &HeaderMap) -> bool {
    if let Some(site) = h.get("sec-fetch-site").and_then(|v| v.to_str().ok()) {
        return matches!(site, "same-origin" | "none");
    }
    // Older browsers: compare Origin's host with Host. No Origin = not a browser form.
    match (
        h.get(header::ORIGIN).and_then(|v| v.to_str().ok()),
        h.get(header::HOST).and_then(|v| v.to_str().ok()),
    ) {
        (None, _) => true,
        (Some(origin), Some(host)) => origin.split("://").nth(1) == Some(host),
        (Some(_), None) => false,
    }
}

// -- errors -------------------------------------------------------------------

pub struct AppError(anyhow::Error);

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

type Res<T> = Result<T, AppError>;

fn render(t: impl Template) -> Res<Html<String>> {
    Ok(Html(t.render()?))
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not found").into_response()
}

/// Back to a page with a short message ("Added 2 × Rice").
fn back(to: &str, msg: impl AsRef<str>) -> Response {
    let sep = if to.contains('?') { '&' } else { '?' };
    Redirect::to(&format!("{to}{sep}msg={}", url_encode(msg.as_ref()))).into_response()
}

/// After a form action: the success message, or the validation error.
fn done(to: &str, r: anyhow::Result<String>) -> Response {
    match r {
        Ok(m) => back(to, m),
        Err(e) => back(to, format!("Not saved: {e}")),
    }
}

fn url_encode(s: &str) -> String {
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

// -- form parsing helpers ---------------------------------------------------------

fn opt_f64(s: &Option<String>) -> anyhow::Result<Option<f64>> {
    match s.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        // Accept a decimal comma: half of Europe types "1,5".
        Some(v) => v
            .replace(',', ".")
            .parse::<f64>()
            .map(Some)
            .map_err(|_| anyhow::anyhow!("{v:?} is not a number")),
    }
}

fn opt_i64(s: &Option<String>) -> Option<i64> {
    s.as_deref().and_then(|v| v.trim().parse().ok())
}

fn opt_date(s: &Option<String>) -> anyhow::Result<Option<NaiveDate>> {
    match s.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(v) => NaiveDate::parse_from_str(v, "%Y-%m-%d")
            .map(Some)
            .map_err(|_| anyhow::anyhow!("{v:?} is not a date")),
    }
}

// -- view models ------------------------------------------------------------------

#[derive(Default, Deserialize)]
pub struct Flash {
    msg: Option<String>,
    undo: Option<String>,
    /// Prefill for the add-stock form (from the shopping list).
    qty: Option<String>,
}

pub struct BatchRow {
    pub image: Option<String>,
    pub id: i64,
    pub item_id: i64,
    pub name: String,
    pub initial: String,
    pub sub: String,
    pub status: &'static str,
    pub when: String,
    pub qty: String,
    pub can_use: bool,
}

impl From<&Dated> for BatchRow {
    fn from(d: &Dated) -> Self {
        let b = &d.batch;
        let mut sub = format!("{} {}", fmt_qty(b.qty), tr(&b.unit));
        if let Some(l) = &b.location_name {
            sub += &format!(" · {l}");
        }
        sub += &format!(" · {}", hdate(b.expires_on.as_deref()));
        Self {
            image: b.image.clone(),
            id: b.id,
            item_id: b.item_id,
            initial: initial(&b.item_name),
            name: b.item_name.clone(),
            sub,
            status: d.status.as_str(),
            when: when(d.days_left),
            qty: fmt_qty(b.qty),
            can_use: b.kind != "document",
        }
    }
}

pub struct ItemRow {
    pub image: Option<String>,
    pub id: i64,
    pub name: String,
    pub initial: String,
    pub sub: String,
    pub low: bool,
}

impl From<&db::Item> for ItemRow {
    fn from(i: &db::Item) -> Self {
        let mut sub = format!("{} {}", fmt_qty(i.total), tr(&i.unit));
        if i.min_qty > 0.0 {
            sub += &format!(" {} {}", tr("of min"), fmt_qty(i.min_qty));
        }
        if let Some(c) = &i.category {
            sub += &format!(" · {}", tr(c));
        }
        if i.next_expiry.is_some() {
            sub += &format!(" · {} {}", tr("next"), hdate(i.next_expiry.as_deref()));
        }
        Self {
            image: i.image.clone(),
            id: i.id,
            initial: initial(&i.name),
            name: i.name.clone(),
            sub,
            low: i.is_low(),
        }
    }
}

pub struct PlanLine {
    pub id: i64,
    pub label: String,
    pub category: String,
    pub mode: String,
    pub unit: String,
    pub amount: String,
    pub progress: String,
    pub pct: u32,
    pub bar: &'static str,
    pub covered: bool,
    pub contributors: String,
}

impl From<&rules::Coverage> for PlanLine {
    fn from(c: &rules::Coverage) -> Self {
        let mut progress = format!(
            "{} / {} {}",
            fmt_qty(c.have),
            fmt_qty(c.need),
            tr(&c.target.unit)
        );
        if let Some(d) = c.days {
            progress += &format!(" · {} {}", fmt_qty(d), tr("days"));
        }
        let contributors = if c.items.is_empty() {
            format!(
                "{} \"{}\"",
                tr("Nothing yet in category"),
                tr(&c.target.category)
            )
        } else {
            c.items
                .iter()
                .map(|x| format!("{} ({} {})", x.name, fmt_qty(x.amount), tr(&c.target.unit)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        Self {
            id: c.target.id,
            label: c.target.label.clone(),
            category: c.target.category.clone(),
            mode: c.target.mode.clone(),
            unit: c.target.unit.clone(),
            amount: fmt_qty(c.target.amount),
            progress,
            pct: pct(c.ratio),
            bar: bar_class(c.ratio),
            covered: c.ratio >= 1.0,
            contributors,
        }
    }
}

/// "€12.50", "CHF 3.20".
pub fn money(v: f64, currency: &str) -> String {
    let sym = match currency {
        "EUR" => "€",
        "USD" => "$",
        "GBP" => "£",
        _ => "",
    };
    if sym.is_empty() {
        format!("{currency} {v:.2}")
    } else {
        format!("{sym}{v:.2}")
    }
}

fn pct(r: f64) -> u32 {
    (r.clamp(0.0, 1.0) * 100.0).round() as u32
}

fn bar_class(r: f64) -> &'static str {
    if r < 0.4 {
        "low"
    } else if r < 0.9 {
        "mid"
    } else {
        ""
    }
}

fn item_unit(i: &db::Item) -> &str {
    if i.unit == "pcs" {
        "piece"
    } else {
        &i.unit
    }
}

fn initial(name: &str) -> String {
    name.chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "?".into())
}

fn when(d: Option<i64>) -> String {
    use crate::i18n::trf;
    match d {
        None => tr("no date"),
        Some(0) => tr("today"),
        Some(1) => tr("tomorrow"),
        Some(d) if d < 0 => trf("{0}d ago", &[&-d]),
        Some(d) if d < 60 => trf("{0}d", &[&d]),
        Some(d) => trf("{0}mo", &[&(d / 30)]),
    }
}

fn hdate(s: Option<&str>) -> String {
    let fmt = tr("%-d %b %Y");
    s.and_then(|v| NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
        .map(|d| d.format(&fmt).to_string())
        .unwrap_or_else(|| tr("no date"))
}

// -- pages ------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "home.html")]
struct HomeT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    expired: usize,
    soon: usize,
    expiring: Vec<BatchRow>,
    low: Vec<ItemRow>,
    plan_pct: u32,
    plan_bar: &'static str,
    plan_days: i64,
    people: i64,
    weakest: Option<String>,
    gaps: Vec<String>,
    recent: Vec<String>,
    month_note: Option<String>,
}

async fn home(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    let today = db::today();
    let snap = rules::Snapshot::load(&s.store, today)?;
    let month = today.format("%Y-%m-01").to_string();
    let (spent, wasted) = (s.store.spending(&month)?, s.store.wasted(&month)?);
    let month_note = match (spent > 0.0, wasted > 0) {
        (false, false) => None,
        (true, false) => Some(format!(
            "{} {}",
            tr("Spent this month:"),
            money(spent, &s.cfg.currency)
        )),
        (false, true) => Some(format!("{} {wasted}×", tr("Thrown away this month:"))),
        (true, true) => Some(format!(
            "{} {} · {} {wasted}×",
            tr("Spent this month:"),
            money(spent, &s.cfg.currency),
            tr("Thrown away:")
        )),
    };
    let exp = snap.expiring(14);
    let plan = rules::coverage_of(&s.store, &snap)?;
    let recent = s
        .store
        .recent(8)?
        .into_iter()
        .map(|e| {
            let verb = match e.action.as_str() {
                "add" => "Added",
                "use" => "Used",
                _ => "Threw away",
            };
            format!(
                "{} {} × {}",
                tr(verb),
                fmt_qty(e.qty.unwrap_or(0.0)),
                e.item_name.unwrap_or_default()
            )
        })
        .collect();
    render(HomeT {
        msg: f.msg,
        tab: "home",
        undo: f.undo,
        expired: exp.iter().filter(|b| b.status == Status::Expired).count(),
        soon: exp.iter().filter(|b| b.status != Status::Expired).count(),
        expiring: exp.iter().map(BatchRow::from).collect(),
        low: snap.low_stock().iter().map(ItemRow::from).collect(),
        plan_pct: pct(plan.ratio),
        plan_bar: bar_class(plan.ratio),
        plan_days: plan.days,
        people: plan.people,
        weakest: plan.weakest_days.map(fmt_qty),
        gaps: {
            // Labels contain commas, so they're separated with "; ".
            let g: Vec<String> = plan.gaps().map(|c| tr(&c.target.label)).collect();
            if g.len() > 3 {
                vec![format!(
                    "{}; +{} {}",
                    g[..3].join("; "),
                    g.len() - 3,
                    tr("more")
                )]
            } else {
                g
            }
        },
        recent,
        month_note,
    })
}

#[derive(Template)]
#[template(path = "items.html")]
struct ItemsT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    q: String,
    kind: String,
    kinds: &'static [&'static str],
    items: Vec<ItemRow>,
}

#[derive(Deserialize)]
struct ItemsQ {
    q: Option<String>,
    kind: Option<String>,
    sort: Option<String>,
    msg: Option<String>,
}

async fn items_page(State(s): St, Query(p): Query<ItemsQ>) -> Res<Html<String>> {
    let q = p.q.unwrap_or_default();
    let kind = p.kind.unwrap_or_default();
    let items = s.store.items(&ItemQuery {
        search: &q,
        kind: &kind,
        sort: Sort::parse(p.sort.as_deref().unwrap_or("name")),
        ..Default::default()
    })?;
    render(ItemsT {
        msg: p.msg,
        tab: "items",
        undo: None,
        items: items.iter().map(ItemRow::from).collect(),
        q,
        kind,
        kinds: KINDS,
    })
}

#[derive(Template)]
#[template(path = "item.html")]
struct ItemT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    item: db::Item,
    total: String,
    min_qty: String,
    batches: Vec<BatchRow>,
    history: Vec<String>,
    locations: Vec<db::Location>,
    attachments: Vec<db::Attachment>,
    prices: Vec<String>,
    extra_barcodes: Vec<String>,
    insights: Vec<String>,
    restock_qty: String,
    expiry_hint: Option<(String, String)>,
    /// Where the batch buttons return to (used by `_batch_row.html`).
    back_to: String,
    /// Medicines: the daily dose form is shown for the medicine category or
    /// when a dose is set.
    show_dose: bool,
    has_dose: bool,
    dose: DoseView,
    /// (id, label, chosen) for the "for" select.
    members: Vec<(i64, String, bool)>,
    forms: &'static [&'static str],
    /// The checklist entry this item counts for.
    preset: Option<&'static str>,
}

#[derive(Default)]
struct DoseView {
    member_id: Option<i64>,
    form: String,
    strength: String,
    per_day: String,
    renew_on: String,
    /// "lasts 14 days"
    days: String,
}

async fn item_page(State(s): St, Path(id): Path<i64>, Query(f): Query<Flash>) -> Res<Response> {
    let Some(item) = s.store.item(id)? else {
        return Ok(not_found());
    };
    let batches = rules::annotate(s.store.batches(Some(id))?, db::today());
    let history = s
        .store
        .history(id, 20)?
        .into_iter()
        .map(|e| {
            let q = e
                .qty
                .map(|q| format!(" {}", fmt_qty(q)))
                .unwrap_or_default();
            let d = e.detail.map(|d| format!(" ({d})")).unwrap_or_default();
            format!("{} · {}{q}{d}", e.ts.replace('T', " "), tr(&e.action))
        })
        .collect();
    let dose = s.store.dose(id)?.map(|d| {
        let usable: f64 = batches
            .iter()
            .filter(|b| b.status != Status::Expired)
            .map(|b| b.batch.qty)
            .sum();
        DoseView {
            days: fmt_qty((usable / d.per_day * 10.0).floor() / 10.0),
            member_id: d.member_id,
            form: d.form,
            strength: d.strength,
            per_day: fmt_qty(d.per_day),
            renew_on: d.renew_on.unwrap_or_default(),
        }
    });
    let has_dose = dose.is_some();
    let dose = dose.unwrap_or_default();
    let show_dose = has_dose || item.category.as_deref() == Some("medicine");
    let members = if show_dose {
        let ms = s.store.members()?;
        let labels = crate::essentials::member_labels(&ms);
        ms.iter()
            .map(|m| (m.id, labels[&m.id].clone(), dose.member_id == Some(m.id)))
            .collect()
    } else {
        Vec::new()
    };
    Ok(render(ItemT {
        msg: f.msg,
        tab: "items",
        undo: f.undo,
        back_to: format!("/items/{id}"),
        show_dose,
        has_dose,
        dose,
        members,
        forms: db::MED_FORMS,
        preset: item
            .preset
            .as_deref()
            .and_then(crate::essentials::find)
            .map(|c| c.label),
        total: fmt_qty(item.total),
        min_qty: fmt_qty(item.min_qty),
        batches: batches.iter().map(BatchRow::from).collect(),
        history,
        locations: s.store.locations()?,
        attachments: s.store.attachments(id)?,
        extra_barcodes: s.store.extra_barcodes(id)?,
        insights: crate::smart::insights(&s.store, &item, db::today())?,
        restock_qty: f
            .qty
            .clone()
            .filter(|q| q.parse::<f64>().is_ok())
            .or_else(|| {
                crate::smart::usual_qty(&s.store, id)
                    .ok()
                    .flatten()
                    .map(fmt_qty)
            })
            .unwrap_or_else(|| "1".into()),
        expiry_hint: if item.kind == "document" {
            None
        } else {
            crate::smart::shelf_life(&s.store, id)?.map(|sl| {
                let d = db::today() + chrono::Days::new(sl.days as u64);
                (sl.label, d.to_string())
            })
        },
        prices: s
            .store
            .prices(id, 10)?
            .into_iter()
            .map(|p| {
                let cur = &s.cfg.currency;
                let per = p
                    .unit_price
                    .filter(|_| p.qty.is_some_and(|q| q != 1.0))
                    .map(|u| format!(" ({} per {})", money(u, cur), item_unit(&item)))
                    .unwrap_or_default();
                format!(
                    "{} · {} × {}{per}",
                    hdate(Some(&p.ts[..10.min(p.ts.len())])),
                    fmt_qty(p.qty.unwrap_or(1.0)),
                    money(p.price, cur)
                )
            })
            .collect(),
        item,
    })?
    .into_response())
}

#[derive(Template)]
#[template(path = "add.html")]
struct AddT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    p: Prefill,
    similar: Vec<ItemRow>,
    kinds: &'static [&'static str],
    units: &'static [&'static str],
    categories: &'static [&'static str],
    locations: Vec<db::Location>,
}

/// Values to prefill the add form with (from a barcode lookup). All strings,
/// all optional; the template escapes them.
#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Prefill {
    msg: Option<String>,
    name: String,
    brand: String,
    barcode: String,
    kind: String,
    category: String,
    size: String,
    size_unit: String,
    qty: String,
    image: String,
    nutriscore: String,
    location_id: String,
    source: String,
    hint: String,
}

async fn add_page(State(s): St, Query(mut p): Query<Prefill>) -> Res<Html<String>> {
    if !p.image.is_empty() && !crate::files::is_safe_name(&p.image) {
        p.image.clear();
    }
    if p.qty.is_empty() {
        p.qty = "1".into();
    }
    let similar = if p.barcode.is_empty() || p.name.is_empty() {
        Vec::new()
    } else {
        s.store
            .similar_items(&p.name, 5)?
            .iter()
            .map(ItemRow::from)
            .collect()
    };
    render(AddT {
        similar,
        msg: p.msg.take(),
        p,
        tab: "add",
        undo: None,
        kinds: KINDS,
        units: UNITS,
        categories: CATEGORIES,
        locations: s.store.locations()?,
    })
}

#[derive(Deserialize)]
struct AddForm {
    name: String,
    kind: Option<String>,
    category: Option<String>,
    unit: Option<String>,
    size: Option<String>,
    size_unit: Option<String>,
    barcode: Option<String>,
    min_qty: Option<String>,
    qty: Option<String>,
    expires_on: Option<String>,
    location_id: Option<String>,
    price: Option<String>,
    brand: Option<String>,
    image: Option<String>,
    nutriscore: Option<String>,
}

async fn add_submit(State(s): St, Form(f): Form<AddForm>) -> Response {
    let r = (|| -> anyhow::Result<(i64, String)> {
        let qty = opt_f64(&f.qty)?.unwrap_or(1.0);
        let expires = opt_date(&f.expires_on)?;
        let loc = opt_i64(&f.location_id);
        let price = opt_f64(&f.price)?;
        // A known barcode adds stock to the existing item instead of a duplicate.
        let existing = match f
            .barcode
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
        {
            Some(code) => s.store.item_by_barcode(code)?,
            // A typed name that already exists: add to it instead of a duplicate.
            None => s.store.item_by_name(&f.name)?,
        };
        let id = match existing {
            Some(it) => it.id,
            None => s.store.add_item(NewItem {
                name: f.name.clone(),
                kind: f.kind.clone(),
                category: f.category.clone(),
                unit: f.unit.clone(),
                size: opt_f64(&f.size)?,
                size_unit: f.size_unit.clone(),
                barcode: f.barcode.clone(),
                min_qty: opt_f64(&f.min_qty)?,
                default_location_id: loc,
                brand: f.brand.clone(),
                image: f.image.clone(),
                nutriscore: f.nutriscore.clone(),
                ..Default::default()
            })?,
        };
        s.store.add_batch(id, qty, expires, loc, price)?;
        Ok((
            id,
            format!("{} {} × {}", tr("Added"), fmt_qty(qty), f.name.trim()),
        ))
    })();
    match r {
        Ok((id, m)) => back(&format!("/items/{id}"), m),
        Err(e) => back("/add", format!("Not saved: {e}")),
    }
}

#[derive(Deserialize)]
struct BatchForm {
    qty: Option<String>,
    expires_on: Option<String>,
    location_id: Option<String>,
    price: Option<String>,
}

async fn add_batch(State(s): St, Path(id): Path<i64>, Form(f): Form<BatchForm>) -> Response {
    let to = format!("/items/{id}");
    done(
        &to,
        (|| {
            let qty = opt_f64(&f.qty)?.unwrap_or(1.0);
            s.store.add_batch(
                id,
                qty,
                opt_date(&f.expires_on)?,
                opt_i64(&f.location_id),
                opt_f64(&f.price)?,
            )?;
            Ok(format!("{} {}", tr("Added"), fmt_qty(qty)))
        })(),
    )
}

#[derive(Deserialize)]
struct UseForm {
    qty: Option<String>,
    batch_id: Option<String>,
    waste: Option<String>,
    back: Option<String>,
}

async fn use_item(State(s): St, Path(id): Path<i64>, Form(f): Form<UseForm>) -> Response {
    let to = safe_back(f.back.clone(), format!("/items/{id}"));
    let r = (|| -> anyhow::Result<(String, Option<String>)> {
        let waste = f.waste.is_some();
        let cp = s.store.checkpoint(id)?;
        let taken = s.store.consume(
            id,
            opt_f64(&f.qty)?.unwrap_or(1.0),
            opt_i64(&f.batch_id),
            waste,
        )?;
        if taken == 0.0 {
            return Ok(("Nothing left to take".into(), None));
        }
        let name = s.store.item(id)?.map(|i| i.name).unwrap_or_default();
        let msg = format!(
            "{} {} × {name}",
            tr(if waste { "Threw away" } else { "Used" }),
            fmt_qty(taken)
        );
        let token = s.store.save_undo(cp, &msg)?;
        Ok((msg, Some(token)))
    })();
    match r {
        Ok((m, Some(t))) => back_undo(&to, m, &t),
        Ok((m, None)) => back(&to, m),
        Err(e) => back(&to, format!("Not saved: {e}")),
    }
}

/// Redirect target from a form: only our own pages (no open redirect).
fn safe_back(b: Option<String>, default: String) -> String {
    b.filter(|b| b.starts_with('/') && !b.starts_with("//") && !b.contains('\\'))
        .unwrap_or(default)
}

fn back_undo(to: &str, msg: impl AsRef<str>, token: &str) -> Response {
    let sep = if to.contains('?') { '&' } else { '?' };
    Redirect::to(&format!(
        "{to}{sep}msg={}&undo={}",
        url_encode(msg.as_ref()),
        url_encode(token)
    ))
    .into_response()
}

async fn undo(State(s): St, Path(token): Path<String>) -> Response {
    match s.store.undo(&token) {
        Ok((Some(id), m)) => back(&format!("/items/{id}"), m),
        Ok((None, m)) => back("/", m),
        Err(e) => back("/", format!("Not undone: {e}")),
    }
}

async fn open_batch(State(s): St, Path(id): Path<i64>) -> Response {
    let r = (|| -> anyhow::Result<(i64, String)> {
        let Some((item_id, _)) = s.store.batch(id)? else {
            anyhow::bail!("no such batch")
        };
        let msg = match s.store.open_batch(id, db::today())? {
            Some(d) => format!("{} {}", tr("Opened: use by"), hdate(Some(&d.to_string()))),
            None => "Marked as opened".into(),
        };
        Ok((item_id, msg))
    })();
    match r {
        Ok((item, m)) => back(&format!("/items/{item}"), m),
        Err(e) => back("/", format!("Not saved: {e}")),
    }
}

#[derive(Deserialize)]
struct MinForm {
    min_qty: Option<String>,
}

async fn set_minimum(State(s): St, Path(id): Path<i64>, Form(f): Form<MinForm>) -> Response {
    done(
        &format!("/items/{id}"),
        (|| {
            s.store
                .set_min_qty(id, opt_f64(&f.min_qty)?.unwrap_or(0.0))?;
            Ok("Minimum saved".into())
        })(),
    )
}

async fn delete_item(State(s): St, Path(id): Path<i64>) -> Response {
    let files = s.store.item_files(id).unwrap_or_default();
    let r = s.store.delete_item(id);
    if r.is_ok() {
        for f in files.iter().filter(|f| crate::files::is_safe_name(f)) {
            let _ = std::fs::remove_file(s.cfg.files_dir().join(f));
        }
    }
    done("/items", r.map(|_| "Deleted".into()))
}

// -- edit item ------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "item_form.html")]
struct ItemFormT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    item: db::Item,
    min_qty: String,
    size: String,
    open_days: String,
    kinds: &'static [&'static str],
    units: &'static [&'static str],
    size_units: &'static [&'static str],
    categories: &'static [&'static str],
    locations: Vec<db::Location>,
}

async fn edit_item_page(
    State(s): St,
    Path(id): Path<i64>,
    Query(f): Query<Flash>,
) -> Res<Response> {
    let Some(item) = s.store.item(id)? else {
        return Ok(not_found());
    };
    Ok(render(ItemFormT {
        msg: f.msg,
        tab: "items",
        undo: None,
        min_qty: if item.min_qty > 0.0 {
            fmt_qty(item.min_qty)
        } else {
            String::new()
        },
        size: item.size.map(fmt_qty).unwrap_or_default(),
        open_days: item.open_days.map(|d| d.to_string()).unwrap_or_default(),
        item,
        kinds: KINDS,
        units: UNITS,
        size_units: &UNITS[1..], // all but "pcs"
        categories: CATEGORIES,
        locations: s.store.locations()?,
    })?
    .into_response())
}

#[derive(Deserialize)]
struct EditForm {
    name: String,
    kind: Option<String>,
    category: Option<String>,
    unit: Option<String>,
    size: Option<String>,
    size_unit: Option<String>,
    barcode: Option<String>,
    brand: Option<String>,
    min_qty: Option<String>,
    open_days: Option<String>,
    default_location_id: Option<String>,
    notes: Option<String>,
}

async fn edit_item(State(s): St, Path(id): Path<i64>, Form(f): Form<EditForm>) -> Response {
    let r = (|| -> anyhow::Result<()> {
        s.store.update_item(
            id,
            NewItem {
                name: f.name.clone(),
                kind: f.kind.clone(),
                category: f.category.clone(),
                unit: f.unit.clone(),
                size: opt_f64(&f.size)?,
                size_unit: f.size_unit.clone(),
                barcode: f.barcode.clone(),
                brand: f.brand.clone(),
                min_qty: opt_f64(&f.min_qty)?,
                open_days: opt_i64(&f.open_days),
                default_location_id: opt_i64(&f.default_location_id),
                notes: f.notes.clone(),
                ..Default::default()
            },
        )
    })();
    match r {
        Ok(()) => back(&format!("/items/{id}"), "Saved"),
        Err(e) => back(&format!("/items/{id}/edit"), format!("Not saved: {e}")),
    }
}

// -- barcodes -----------------------------------------------------------------------

#[derive(Deserialize)]
struct LinkForm {
    code: String,
    item_id: i64,
}

async fn link_barcode(State(s): St, Form(f): Form<LinkForm>) -> Response {
    let to = format!("/items/{}", f.item_id);
    done(
        &to,
        s.store
            .link_barcode(&f.code, f.item_id)
            .map(|_| format!("Barcode {} linked", f.code.trim())),
    )
}

#[derive(Deserialize)]
struct CodeForm {
    code: String,
}

async fn unlink_barcode(State(s): St, Path(id): Path<i64>, Form(f): Form<CodeForm>) -> Response {
    done(
        &format!("/items/{id}"),
        s.store
            .unlink_barcode(&f.code)
            .map(|_| "Barcode removed".into()),
    )
}

// -- attachments ------------------------------------------------------------------------

const MAX_UPLOAD: usize = 25 * 1024 * 1024;

async fn upload(State(s): St, Path(id): Path<i64>, mut mp: axum::extract::Multipart) -> Response {
    let to = format!("/items/{id}");
    let mut as_picture = false;
    let mut file: Option<(String, String, axum::body::Bytes)> = None;
    loop {
        match mp.next_field().await {
            Ok(Some(field)) => match field.name() {
                Some("as_picture") => as_picture = true,
                Some("file") => {
                    let fname = field.file_name().unwrap_or("file").to_string();
                    let ctype = field.content_type().unwrap_or("").to_string();
                    match field.bytes().await {
                        Ok(b) => file = Some((fname, ctype, b)),
                        Err(_) => return back(&to, "Upload failed (max 25 MB)"),
                    }
                }
                _ => {}
            },
            Ok(None) => break,
            Err(_) => return back(&to, "Upload failed (max 25 MB)"),
        }
    }
    let Some((fname, _ctype, bytes)) = file else {
        return back(&to, "Choose a file");
    };
    // Trust the bytes, not the browser's label: only real images and PDFs.
    let Some((mime, ext)) = sniff(&bytes) else {
        return back(&to, "Only photos (JPEG, PNG, WebP) and PDFs");
    };
    if bytes.len() > MAX_UPLOAD {
        return back(&to, "File too large (max 25 MB)");
    }
    let r = (|| -> anyhow::Result<&'static str> {
        if s.store.item(id)?.is_none() {
            anyhow::bail!("no such item");
        }
        let stored = crate::files::new_name("att", ext);
        crate::files::write(&s.cfg.files_dir(), &stored, &bytes)?;
        if as_picture && mime.starts_with("image/") {
            s.store.set_image(id, Some(&stored))?;
            return Ok("Picture updated");
        }
        let clean: String = fname
            .chars()
            .filter(|c| !c.is_control() && *c != '/' && *c != '\\')
            .take(120)
            .collect();
        s.store
            .add_attachment(id, &clean, &stored, mime, bytes.len() as i64)?;
        Ok("Attached")
    })();
    done(&to, r.map(String::from))
}

/// File type from its first bytes.
fn sniff(b: &[u8]) -> Option<(&'static str, &'static str)> {
    if b.starts_with(b"\xFF\xD8\xFF") {
        Some(("image/jpeg", "jpg"))
    } else if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("image/png", "png"))
    } else if b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some(("image/webp", "webp"))
    } else if b.starts_with(b"%PDF-") {
        Some(("application/pdf", "pdf"))
    } else {
        None
    }
}

async fn get_attachment(State(s): St, Path(id): Path<i64>) -> Res<Response> {
    let Some(a) = s.store.attachment(id)? else {
        return Ok(not_found());
    };
    if !crate::files::is_safe_name(&a.stored) {
        return Ok(not_found());
    }
    let Ok(bytes) = tokio::fs::read(s.cfg.files_dir().join(&a.stored)).await else {
        return Ok(not_found());
    };
    let ascii: String = a
        .filename
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || ".-_ ".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut res = (
        [
            (
                header::CONTENT_TYPE,
                crate::files::content_type(&a.stored).to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{ascii}\""),
            ),
        ],
        bytes,
    )
        .into_response();
    // A PDF opened inline must not run scripts in our origin.
    res.headers_mut().insert("content-security-policy", HeaderValue::from_static("sandbox; default-src 'none'; img-src 'self'; style-src 'unsafe-inline'; plugin-types application/pdf"));
    res.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(res)
}

async fn delete_attachment(State(s): St, Path(id): Path<i64>) -> Res<Response> {
    let Some(a) = s.store.attachment(id)? else {
        return Ok(not_found());
    };
    s.store.delete_attachment(id)?;
    if crate::files::is_safe_name(&a.stored) {
        let _ = std::fs::remove_file(s.cfg.files_dir().join(&a.stored));
    }
    Ok(back(&format!("/items/{}", a.item_id), "Removed"))
}

// -- locations --------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "locations.html")]
struct LocationsT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    locations: Vec<LocationRow>,
}

struct LocationRow {
    id: i64,
    name: String,
    path: String,
    count: i64,
    parent_id: Option<i64>,
}

async fn locations_page(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    let counts = s.store.location_counts()?;
    let locations = s
        .store
        .locations()?
        .into_iter()
        .map(|l| LocationRow {
            count: counts.get(&l.id).copied().unwrap_or(0),
            id: l.id,
            name: l.name,
            path: l.path,
            parent_id: l.parent_id,
        })
        .collect();
    render(LocationsT {
        msg: f.msg,
        tab: "items",
        undo: None,
        locations,
    })
}

#[derive(Deserialize)]
struct LocForm {
    name: String,
    parent_id: Option<String>,
    delete: Option<String>,
}

async fn add_location(State(s): St, Form(f): Form<LocForm>) -> Response {
    done(
        "/locations",
        s.store
            .add_location(&f.name, opt_i64(&f.parent_id))
            .map(|_| "Added".into()),
    )
}

async fn edit_location(State(s): St, Path(id): Path<i64>, Form(f): Form<LocForm>) -> Response {
    if f.delete.is_some() {
        return done(
            "/locations",
            s.store
                .delete_location(id)
                .map(|_| "Deleted (its stock is now without a location)".into()),
        );
    }
    done(
        "/locations",
        s.store
            .rename_location(id, &f.name, opt_i64(&f.parent_id))
            .map(|_| "Saved".into()),
    )
}

// -- calendar ------------------------------------------------------------------------

/// Expiry and renewal dates as an iCal feed, for the phone's calendar.
async fn calendar(State(s): St) -> Res<Response> {
    let esc = |t: &str| {
        t.replace('\\', "\\\\")
            .replace(';', "\\;")
            .replace(',', "\\,")
            .replace('\n', "\\n")
    };
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let mut out = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".into(),
        "PRODID:-//Vorrat//Vorrat//EN".into(),
        "X-WR-CALNAME:Vorrat expiry dates".into(),
        "CALSCALE:GREGORIAN".into(),
    ];
    for b in s.store.dated_batches()? {
        let Some(d) = b
            .expires_on
            .as_deref()
            .and_then(|e| NaiveDate::parse_from_str(e, "%Y-%m-%d").ok())
        else {
            continue;
        };
        let mut summary = format!(
            "{}: {}",
            if b.kind == "document" {
                "Renew"
            } else {
                "Expires"
            },
            b.item_name
        );
        if b.kind != "document" {
            summary += &format!(" ({} {})", fmt_qty(b.qty), b.unit);
        }
        out.extend([
            "BEGIN:VEVENT".to_string(),
            format!("UID:vorrat-batch-{}@vorrat", b.id),
            format!("DTSTAMP:{stamp}"),
            format!("DTSTART;VALUE=DATE:{}", d.format("%Y%m%d")),
            format!(
                "DTEND;VALUE=DATE:{}",
                d.succ_opt().unwrap_or(d).format("%Y%m%d")
            ),
            format!("SUMMARY:{}", esc(&summary)),
            format!(
                "DESCRIPTION:{}",
                esc(b.location_name.as_deref().unwrap_or(""))
            ),
            "END:VEVENT".into(),
        ]);
    }
    out.push("END:VCALENDAR".into());
    Ok((
        [(header::CONTENT_TYPE, "text/calendar; charset=utf-8")],
        out.join("\r\n") + "\r\n",
    )
        .into_response())
}

#[derive(Deserialize)]
struct BackForm {
    back: Option<String>,
}

async fn delete_batch(State(s): St, Path(id): Path<i64>, Form(f): Form<BackForm>) -> Response {
    let to = safe_back(f.back, "/".into());
    let r = (|| -> anyhow::Result<Option<String>> {
        let Some((item_id, _)) = s.store.batch(id)? else {
            return Ok(None);
        };
        let cp = s.store.checkpoint(item_id)?;
        s.store.delete_batch(id)?;
        Ok(Some(s.store.save_undo(cp, "Batch removed")?))
    })();
    match r {
        Ok(Some(t)) => back_undo(&to, "Batch removed", &t),
        Ok(None) => back(&to, "Already gone"),
        Err(e) => back(&to, format!("Not saved: {e}")),
    }
}

// -- scanning -----------------------------------------------------------------------

#[derive(Template)]
#[template(path = "scan.html")]
struct ScanT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    mode: &'static str,
}

#[derive(Deserialize)]
struct ScanQ {
    mode: Option<String>,
    msg: Option<String>,
}

async fn scan_page(Query(q): Query<ScanQ>) -> Res<Html<String>> {
    let mode = if q.mode.as_deref() == Some("use") {
        "use"
    } else {
        "add"
    };
    render(ScanT {
        msg: q.msg,
        tab: "add",
        undo: None,
        mode,
    })
}

#[derive(Deserialize)]
struct CodeQ {
    code: String,
    mode: Option<String>,
}

/// After a scan (or typed code): known item → its page (or use one up);
/// unknown → Open Food Facts → prefilled add form.
async fn scan_code(State(s): St, Query(q): Query<CodeQ>) -> Res<Response> {
    let code = q.code.trim();
    if code.is_empty() {
        return Ok(Redirect::to("/scan").into_response());
    }
    let use_up = q.mode.as_deref() == Some("use");
    if let Some(it) = s.store.item_by_barcode(code)? {
        let to = format!("/items/{}", it.id);
        if use_up {
            if it.total <= 0.0 {
                return Ok(back(&to, format!("{}: none in stock", it.name)));
            }
            s.store.consume(it.id, 1.0, None, false)?;
            return Ok(back(&to, format!("{} 1 × {}", tr("Used"), it.name)));
        }
        return Ok(Redirect::to(&to).into_response());
    }
    if use_up {
        return Ok(back(
            "/scan?mode=use",
            format!("{code} is not in Vorrat yet"),
        ));
    }
    let mut params: Vec<(&str, String)> = vec![("barcode", code.to_string())];
    let found = if s.cfg.lookup_enabled {
        crate::lookup::lookup(code, &s.cfg.files_dir(), &s.cfg.user_agent()).await
    } else {
        None
    };
    match found {
        Some(f) => {
            params.push(("name", f.name));
            params.push(("kind", f.kind));
            params.push(("source", f.source));
            for (k, v) in [
                ("brand", f.brand),
                ("category", f.category.clone()),
                ("size_unit", f.size_unit),
                ("image", f.image),
                ("nutriscore", f.nutriscore),
            ] {
                if let Some(v) = v {
                    params.push((k, v));
                }
            }
            if let Some(sz) = f.size {
                params.push(("size", fmt_qty(sz)));
            }
            if let Some(n) = f.pack {
                params.push(("qty", n.to_string()));
            }
            if let Some(g) =
                crate::smart::guess_location(&s.store, f.category.as_deref(), f.frozen)?
            {
                params.push(("location_id", g.id.to_string()));
                params.push((
                    "hint",
                    format!("{} {}: {}.", tr("Location"), g.path, g.reason),
                ));
            }
        }
        None => params.push(("msg", "Not found in Open Food Facts. Type the name.".into())),
    }
    let qs: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{k}={}", url_encode(v)))
        .collect();
    Ok(Redirect::to(&format!("/add?{}", qs.join("&"))).into_response())
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

// -- plan ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "plan.html")]
struct PlanT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    people: i64,
    pets: i64,
    days: i64,
    pct: u32,
    weakest: Option<String>,
    lines: Vec<PlanLine>,
    has_gaps: bool,
    categories: &'static [&'static str],
    units: &'static [&'static str],
    presets: &'static [(i64, &'static str)],
    modes: &'static [(&'static str, &'static str)],
    medicine_days: Option<String>,
    suggestions: Vec<SuggestRow>,
    /// (have, of) for the first-aid kit and the household essentials.
    first_aid: (usize, usize),
    essentials: (usize, usize),
}

struct SuggestRow {
    key: &'static str,
    label: &'static str,
    /// The people and pets it is for.
    who: String,
    named_by: String,
    candidates: Vec<(i64, String)>,
}

/// How a target scales, with the text the plan page shows for it.
const MODE_LABELS: &[(&str, &str)] = &[
    ("per_person_day", "per person per day"),
    ("per_person", "per person"),
    ("per_pet_day", "per pet per day"),
    ("per_pet", "per pet"),
    ("fixed", "fixed total"),
];

async fn plan_page(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    use crate::essentials as es;
    let snap = rules::Snapshot::load(&s.store, db::today())?;
    let plan = rules::coverage_of(&s.store, &snap)?;
    let count = |list| {
        let c = es::checklist(list, &snap, plan.people);
        (c.iter().filter(|x| x.ok()).count(), c.len())
    };
    let has_gaps = plan.gaps().next().is_some();
    render(PlanT {
        msg: f.msg,
        tab: "plan",
        undo: None,
        people: plan.people,
        pets: plan.pets,
        days: plan.days,
        pct: pct(plan.ratio),
        weakest: plan.weakest_days.map(fmt_qty),
        has_gaps,
        lines: plan.lines.iter().map(PlanLine::from).collect(),
        categories: CATEGORIES,
        units: UNITS,
        presets: PLAN_PRESETS,
        modes: MODE_LABELS,
        medicine_days: plan.medicine_days.map(fmt_qty),
        first_aid: count(es::FIRST_AID),
        essentials: count(es::ESSENTIALS),
        suggestions: es::suggestions(
            &s.store.members()?,
            &snap,
            &s.store.dismissed(&db::today().to_string())?,
        )
        .into_iter()
        .map(|g| SuggestRow {
            key: g.state.check.key,
            label: g.state.check.label,
            who: g.who.join(", "),
            named_by: g.state.check.named_by.join(", "),
            candidates: snap
                .items
                .iter()
                .filter(|i| {
                    i.preset.is_none() && i.category.as_deref() == Some(g.state.check.category)
                })
                .map(|i| (i.id, i.name.clone()))
                .collect(),
        })
        .collect(),
    })
}

#[derive(Template)]
#[template(path = "plan_print.html")]
struct PlanPrintT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    people: i64,
    days: i64,
    today: String,
    pct: u32,
    weakest: Option<String>,
    lines: Vec<PrintLine>,
    extras: &'static [&'static str],
    soon: Vec<PrintBatch>,
}

struct PrintLine {
    label: String,
    need: String,
    have: String,
    missing: String,
    covered: bool,
}

struct PrintBatch {
    name: String,
    qty: String,
    when: String,
}

/// Things every European civil-protection list mentions that aren't stock you
/// count by category.
const PRINT_EXTRAS: &[&str] = &[
    "Torch and spare batteries",
    "Battery or wind-up radio",
    "First-aid kit",
    "Personal medicines",
    "Copies of documents, in a waterproof bag",
    "Some cash in small notes",
    "Charged power bank",
    "Candles, matches, lighter",
    "Hygiene items and toilet paper",
    "Warm blankets or sleeping bags",
];

async fn plan_print(State(s): St) -> Res<Html<String>> {
    let today = db::today();
    let plan = rules::coverage(&s.store, today)?;
    let unit = |v: f64, u: &str| format!("{} {u}", fmt_qty(v));
    let soon = rules::expiring(&s.store, today, plan.days)?
        .into_iter()
        .filter(|b| b.days_left.is_some_and(|d| d >= 0))
        .map(|b| PrintBatch {
            qty: unit(b.batch.qty, &b.batch.unit),
            when: b.batch.expires_on.clone().unwrap_or_default(),
            name: b.batch.item_name,
        })
        .collect();
    render(PlanPrintT {
        msg: None,
        tab: "plan",
        undo: None,
        people: plan.people,
        days: plan.days,
        today: today.to_string(),
        pct: pct(plan.ratio),
        weakest: plan.weakest_days.map(fmt_qty),
        lines: plan
            .lines
            .iter()
            .map(|c| PrintLine {
                label: c.target.label.clone(),
                need: unit(c.need, &c.target.unit),
                have: unit(c.have, &c.target.unit),
                missing: if c.missing > 0.0 {
                    unit(c.missing, &c.target.unit)
                } else {
                    String::new()
                },
                covered: c.missing <= 0.0,
            })
            .collect(),
        extras: PRINT_EXTRAS,
        soon,
    })
}

/// Days of self-sufficiency that public bodies in Europe recommend. Only the
/// length: quantities stay with the household's own targets.
const PLAN_PRESETS: &[(i64, &str)] = &[
    (3, "72 hours (EU)"),
    (7, "1 week (Sweden, Switzerland)"),
    (10, "10 days (Germany, BBK)"),
    (14, "2 weeks"),
];

#[derive(Deserialize)]
struct PlanSettings {
    days: Option<String>,
}

/// The number of days. Who the plan is for comes from the household page.
async fn plan_settings(State(s): St, Form(f): Form<PlanSettings>) -> Response {
    done(
        "/plan",
        (|| {
            let days = opt_i64(&f.days)
                .filter(|d| (1..=365).contains(d))
                .ok_or_else(|| anyhow::anyhow!("days must be 1 to 365"))?;
            s.store.set_setting("days", &days.to_string())?;
            Ok(format!("Plan: {days} days"))
        })(),
    )
}

// -- household profile --------------------------------------------------------

#[derive(Template)]
#[template(path = "household.html")]
struct HouseholdT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    people: Vec<MemberRow>,
    pets: Vec<MemberRow>,
    person_types: &'static [&'static str],
    pet_types: &'static [&'static str],
    needs: &'static [&'static str],
    pet_target: bool,
}

struct MemberRow {
    id: i64,
    /// The name, or the translated type with a number ("Adult 2").
    label: String,
    name: String,
    r#type: String,
    /// Every need with whether this person has it, for the checkboxes.
    needs: Vec<(&'static str, bool)>,
    /// Translated, for the summary line.
    needs_text: String,
}

async fn household_page(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    let members = s.store.members()?;
    let labels = crate::essentials::member_labels(&members);
    let rows = |kind: &str| -> Vec<MemberRow> {
        members
            .iter()
            .filter(|m| m.kind == kind)
            .map(|m| MemberRow {
                id: m.id,
                label: labels[&m.id].clone(),
                name: m.name.clone().unwrap_or_default(),
                r#type: m.r#type.clone(),
                needs_text: m.needs.iter().map(|x| tr(x)).collect::<Vec<_>>().join(", "),
                needs: db::NEEDS
                    .iter()
                    .map(|n| (*n, m.needs.iter().any(|x| x == n)))
                    .collect(),
            })
            .collect()
    };
    let pet_target = s
        .store
        .targets()?
        .iter()
        .any(|t| t.mode.starts_with("per_pet"));
    render(HouseholdT {
        msg: f.msg,
        tab: "plan",
        undo: None,
        people: rows("person"),
        pets: rows("pet"),
        person_types: db::PERSON_TYPES,
        pet_types: db::PET_TYPES,
        needs: db::NEEDS,
        pet_target,
    })
}

/// Form fields as pairs: needs arrive as repeated `need=` checkboxes.
fn member_fields(f: &[(String, String)]) -> (String, String, String, Vec<String>, bool) {
    let get = |k: &str| {
        f.iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let needs = f
        .iter()
        .filter(|(n, _)| n == "need")
        .map(|(_, v)| v.clone())
        .collect();
    (
        get("kind"),
        get("type"),
        get("name"),
        needs,
        f.iter().any(|(n, _)| n == "delete"),
    )
}

async fn add_member(State(s): St, Form(f): Form<Vec<(String, String)>>) -> Response {
    let (kind, r#type, name, needs, _) = member_fields(&f);
    done(
        "/household",
        s.store
            .add_member(&kind, &r#type, &name, &needs)
            .map(|_| "Added to the household".into()),
    )
}

async fn edit_member(
    State(s): St,
    Path(id): Path<i64>,
    Form(f): Form<Vec<(String, String)>>,
) -> Response {
    let (_, r#type, name, needs, delete) = member_fields(&f);
    done(
        "/household",
        if delete {
            s.store
                .delete_member(id)
                .map(|_| "Removed from the household".into())
        } else {
            s.store
                .update_member(id, &r#type, &name, &needs)
                .map(|_| "Saved".into())
        },
    )
}

// -- medicine and essentials ------------------------------------------------------

#[derive(Template)]
#[template(path = "essentials.html")]
struct EssentialsT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    plan_days: i64,
    /// One group per person or pet who takes something, then "not assigned".
    groups: Vec<MedGroup>,
    /// Medicine-category items without a daily dose.
    undosed: Vec<(i64, String)>,
    first_aid: Vec<CheckRow>,
    first_aid_ok: usize,
    essentials: Vec<CheckRow>,
    essentials_ok: usize,
    members: Vec<(i64, String)>,
    forms: &'static [&'static str],
}

struct MedGroup {
    who: String,
    meds: Vec<MedRow>,
}

struct MedRow {
    id: i64,
    name: String,
    /// "400 mg · tablet", translated form.
    detail: String,
    /// "28 pcs · 2 a day"
    stock: String,
    days: String,
    pct: u32,
    bar: &'static str,
    /// "Renew prescription by 12 Dec", with whether it is soon or overdue.
    renew: Option<(String, bool)>,
    expiry: Option<String>,
}

struct CheckRow {
    key: &'static str,
    label: &'static str,
    ok: bool,
    expired: bool,
    /// "1 / 3" for per-person entries; empty otherwise.
    count: String,
    items: Vec<(i64, String)>,
    named_by: String,
    /// Items that could stand for it: same category, not linked yet.
    candidates: Vec<(i64, String)>,
}

fn check_rows(
    states: Vec<crate::essentials::CheckState>,
    items: &[db::Item],
) -> (Vec<CheckRow>, usize) {
    let ok = states.iter().filter(|c| c.ok()).count();
    let rows = states
        .into_iter()
        .map(|c| CheckRow {
            key: c.check.key,
            label: c.check.label,
            ok: c.ok(),
            expired: c.expired,
            count: if c.check.per_person {
                format!("{} / {}", fmt_qty(c.have), fmt_qty(c.need))
            } else {
                String::new()
            },
            named_by: c.check.named_by.join(", "),
            candidates: items
                .iter()
                .filter(|i| i.preset.is_none() && i.category.as_deref() == Some(c.check.category))
                .map(|i| (i.id, i.name.clone()))
                .collect(),
            items: c.items,
        })
        .collect();
    (rows, ok)
}

async fn essentials_page(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    use crate::essentials as es;
    let today = db::today();
    let snap = rules::Snapshot::load(&s.store, today)?;
    let members = s.store.members()?;
    let labels = es::member_labels(&members);
    let plan_days = s.store.setting_int("days", 14).max(1);
    let meds = es::medicines(&s.store, &snap)?;
    let mut groups: Vec<MedGroup> = Vec::new();
    let mut order: Vec<Option<i64>> = members.iter().map(|m| Some(m.id)).collect();
    order.push(None);
    for who in order {
        let rows: Vec<MedRow> = meds
            .iter()
            .filter(|m| m.dose.member_id.filter(|id| labels.contains_key(id)) == who)
            .map(|m| {
                let ratio = m.days / plan_days as f64;
                let detail = [m.dose.strength.clone(), tr(&m.dose.form)]
                    .into_iter()
                    .filter(|x| !x.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ");
                MedRow {
                    id: m.item.id,
                    name: m.item.name.clone(),
                    detail,
                    stock: format!(
                        "{} {} · {}",
                        fmt_qty(m.have),
                        tr(&m.item.unit),
                        crate::i18n::trf("{0} a day", &[&fmt_qty(m.dose.per_day)])
                    ),
                    days: fmt_qty(m.days),
                    pct: pct(ratio),
                    bar: bar_class(ratio),
                    renew: m.dose.renew_on.as_deref().map(|d| {
                        (
                            hdate(Some(d)),
                            m.renew_in.is_some_and(|r| r <= es::MED_REMIND_DAYS),
                        )
                    }),
                    expiry: m.next_expiry.as_deref().map(|d| hdate(Some(d))),
                }
            })
            .collect();
        if rows.is_empty() {
            continue;
        }
        groups.push(MedGroup {
            who: match who {
                Some(id) => labels[&id].clone(),
                None => tr("Not assigned to anyone"),
            },
            meds: rows,
        });
    }
    let dosed: std::collections::HashSet<i64> = meds.iter().map(|m| m.item.id).collect();
    let undosed = snap
        .items
        .iter()
        .filter(|i| i.category.as_deref() == Some("medicine") && !dosed.contains(&i.id))
        .map(|i| (i.id, i.name.clone()))
        .collect();
    let people = members.iter().filter(|m| m.kind == "person").count() as i64;
    let (first_aid, first_aid_ok) =
        check_rows(es::checklist(es::FIRST_AID, &snap, people), &snap.items);
    let (essentials, essentials_ok) =
        check_rows(es::checklist(es::ESSENTIALS, &snap, people), &snap.items);
    render(EssentialsT {
        msg: f.msg,
        tab: "plan",
        undo: None,
        plan_days,
        groups,
        undosed,
        first_aid,
        first_aid_ok,
        essentials,
        essentials_ok,
        members: members
            .iter()
            .map(|m| (m.id, labels[&m.id].clone()))
            .collect(),
        forms: db::MED_FORMS,
    })
}

#[derive(Deserialize)]
struct DoseForm {
    name: Option<String>,
    unit: Option<String>,
    member_id: Option<String>,
    form: Option<String>,
    strength: Option<String>,
    per_day: Option<String>,
    renew_on: Option<String>,
    delete: Option<String>,
}

async fn add_medicine(State(s): St, Form(f): Form<DoseForm>) -> Response {
    let r = (|| {
        let per_day = opt_f64(&f.per_day)?.unwrap_or(0.0);
        s.store.add_medicine(
            f.name.as_deref().unwrap_or(""),
            f.unit.as_deref().unwrap_or("pcs"),
            opt_i64(&f.member_id),
            f.form.as_deref().unwrap_or(""),
            f.strength.as_deref().unwrap_or(""),
            per_day,
            opt_date(&f.renew_on)?,
        )
    })();
    match r {
        // Next: how much is in the cupboard, and until when it keeps.
        Ok(id) => back(
            &format!("/items/{id}"),
            "Medicine added. Now add the stock you have.",
        )
        .into_response(),
        Err(e) => back("/essentials", format!("Not saved: {e}")),
    }
}

async fn set_dose(State(s): St, Path(id): Path<i64>, Form(f): Form<DoseForm>) -> Response {
    done(
        &format!("/items/{id}"),
        (|| {
            if f.delete.is_some() {
                s.store.delete_dose(id)?;
                return Ok("Daily dose removed".into());
            }
            s.store.set_dose(
                id,
                opt_i64(&f.member_id),
                f.form.as_deref().unwrap_or(""),
                f.strength.as_deref().unwrap_or(""),
                opt_f64(&f.per_day)?.unwrap_or(0.0),
                opt_date(&f.renew_on)?,
            )?;
            Ok("Saved".into())
        })(),
    )
}

#[derive(Deserialize)]
struct CheckForm {
    /// `add`, `shop`, `link` (with `item_id`), or `dismiss` for a suggestion.
    action: String,
    item_id: Option<String>,
}

async fn check_action(State(s): St, Path(key): Path<String>, Form(f): Form<CheckForm>) -> Response {
    let Some(c) = crate::essentials::find(&key) else {
        return not_found();
    };
    let label = tr(c.label);
    // Suggestions live on the plan page, the checklists on their own page.
    let suggestion = !c.applies_to.is_empty();
    let page = if suggestion { "/plan" } else { "/essentials" };
    match f.action.as_str() {
        "add" => {
            let r = s.store.add_item(NewItem {
                name: label,
                kind: Some(
                    if c.category == "documents" {
                        "document"
                    } else {
                        "supply"
                    }
                    .into(),
                ),
                category: Some(c.category.into()),
                ..Default::default()
            });
            match r.and_then(|id| s.store.set_preset(id, Some(c.key)).map(|_| id)) {
                Ok(id) => back(
                    &format!("/items/{id}"),
                    "Added. Now add the stock you have.",
                ),
                Err(e) => back(page, format!("Not saved: {e}")),
            }
        }
        "shop" => done(
            page,
            s.store
                .add_shopping(&label, None, None)
                .map(|_| "Added to the shopping list".into()),
        ),
        "link" => done(
            page,
            (|| {
                let id = opt_i64(&f.item_id).ok_or_else(|| anyhow::anyhow!("pick an item"))?;
                s.store.set_preset(id, Some(c.key))?;
                Ok("Saved".into())
            })(),
        ),
        // "Not needed" is for good; it comes back only if the data is reset.
        "dismiss" if suggestion => done(
            page,
            s.store
                .dismiss(&crate::essentials::dismiss_key(c.key), "9999-12-31")
                .map(|_| "Hidden".into()),
        ),
        _ => not_found(),
    }
}

async fn clear_preset(State(s): St, Path(id): Path<i64>) -> Response {
    done(
        &format!("/items/{id}"),
        s.store.set_preset(id, None).map(|_| "Saved".into()),
    )
}

#[derive(Deserialize)]
struct TargetForm {
    label: String,
    category: String,
    mode: String,
    amount: Option<String>,
    unit: String,
    delete: Option<String>,
}

async fn add_target(State(s): St, Form(f): Form<TargetForm>) -> Response {
    done(
        "/plan",
        (|| {
            let amount = opt_f64(&f.amount)?.unwrap_or(0.0);
            s.store
                .add_target(&f.label, &f.category, &f.mode, amount, &f.unit)?;
            Ok("Target added".into())
        })(),
    )
}

async fn edit_target(State(s): St, Path(id): Path<i64>, Form(f): Form<TargetForm>) -> Response {
    done(
        "/plan",
        (|| {
            if f.delete.is_some() {
                s.store.delete_target(id)?;
                return Ok("Target removed".into());
            }
            let amount = opt_f64(&f.amount)?.unwrap_or(0.0);
            s.store
                .update_target(id, &f.label, &f.category, &f.mode, amount, &f.unit)?;
            Ok("Target saved".into())
        })(),
    )
}

async fn plan_to_shopping(State(s): St) -> Response {
    done(
        "/shopping",
        (|| {
            let plan = rules::coverage(&s.store, db::today())?;
            let open: Vec<String> = s
                .store
                .shopping()?
                .into_iter()
                .filter(|e| !e.done)
                .map(|e| e.label)
                .collect();
            let mut n = 0;
            for c in plan.gaps() {
                let label = format!(
                    "{} ({} {} missing)",
                    c.target.label,
                    fmt_qty(c.missing),
                    c.target.unit
                );
                if !open.iter().any(|l| l.starts_with(&c.target.label)) {
                    s.store.add_shopping(&label, None, Some(c.missing))?;
                    n += 1;
                }
            }
            Ok(format!("{n} gaps added"))
        })(),
    )
}

// -- shopping -----------------------------------------------------------------------

#[derive(Template)]
#[template(path = "shopping.html")]
struct ShoppingT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    entries: Vec<db::ShoppingEntry>,
    any_done: bool,
    suggestions: Vec<(i64, String, String)>,
    low_missing: usize,
}

async fn shopping_page(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    let entries = s.store.shopping()?;
    let listed: Vec<i64> = entries
        .iter()
        .filter(|e| !e.done)
        .filter_map(|e| e.item_id)
        .collect();
    let low_missing = rules::low_stock(&s.store)?
        .iter()
        .filter(|i| !listed.contains(&i.id))
        .count();
    let suggestions = crate::smart::shopping_suggestions(&s.store, db::today())?
        .into_iter()
        .map(|g| (g.item.id, g.item.name, g.reason))
        .collect();
    render(ShoppingT {
        msg: f.msg,
        tab: "shopping",
        undo: None,
        any_done: entries.iter().any(|e| e.done),
        entries,
        suggestions,
        low_missing,
    })
}

#[derive(Deserialize)]
struct ShoppingForm {
    label: String,
    item_id: Option<i64>,
}

async fn add_shopping(State(s): St, Form(f): Form<ShoppingForm>) -> Response {
    done(
        "/shopping",
        s.store
            .add_shopping(&f.label, f.item_id, None)
            .map(|_| "Added".into()),
    )
}

/// Everything running low that isn't on the list yet.
async fn shopping_add_low(State(s): St) -> Response {
    done(
        "/shopping",
        (|| {
            let listed: Vec<i64> = s
                .store
                .shopping()?
                .into_iter()
                .filter(|e| !e.done)
                .filter_map(|e| e.item_id)
                .collect();
            let mut n = 0;
            for it in rules::low_stock(&s.store)?
                .into_iter()
                .filter(|i| !listed.contains(&i.id))
            {
                s.store
                    .add_shopping(&it.name, Some(it.id), Some(it.min_qty - it.total))?;
                n += 1;
            }
            Ok(format!("{n} added"))
        })(),
    )
}

/// A bought entry → the item's add-stock form, prefilled; the entry goes away.
async fn shopping_to_stock(State(s): St, Path(id): Path<i64>) -> Response {
    let r = (|| -> anyhow::Result<Option<(i64, String, f64)>> {
        let Some(e) = s.store.shopping()?.into_iter().find(|e| e.id == id) else {
            return Ok(None);
        };
        let Some(item) = e.item_id else {
            return Ok(None);
        };
        s.store.delete_shopping(id)?;
        let qty = e.qty.filter(|q| *q > 0.0).map(f64::ceil).unwrap_or(1.0);
        Ok(Some((item, e.item_name.unwrap_or(e.label), qty)))
    })();
    match r {
        Ok(Some((item, name, qty))) => Redirect::to(&format!(
            "/items/{item}?qty={}&msg={}#add",
            fmt_qty(qty),
            url_encode(&format!("Bought {name}: add the expiry date and save"))
        ))
        .into_response(),
        Ok(None) => back("/shopping", "Not linked to an item"),
        Err(e) => back("/shopping", format!("Not saved: {e}")),
    }
}

async fn dismiss_suggestion(State(s): St, Path(id): Path<i64>) -> Response {
    done(
        "/shopping",
        crate::smart::dismiss_suggestion(&s.store, id, db::today())
            .map(|_| "Hidden for 14 days".into()),
    )
}

async fn toggle_shopping(State(s): St, Path(id): Path<i64>) -> Response {
    match s.store.toggle_shopping(id) {
        Ok(()) => Redirect::to("/shopping").into_response(),
        Err(e) => back("/shopping", format!("Not saved: {e}")),
    }
}

async fn clear_shopping(State(s): St) -> Response {
    done(
        "/shopping",
        s.store.clear_shopping_done().map(|_| "Cleared".into()),
    )
}

// -- settings -----------------------------------------------------------------------

#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    push: Option<String>,
    digest_time: String,
    backup: Option<String>,
    backup_time: String,
    lookup: bool,
    version: &'static str,
    lang: String,
    langs: Vec<(&'static str, &'static str)>,
    signed_in: bool,
}

async fn settings_page(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    let c = &s.cfg;
    render(SettingsT {
        msg: f.msg,
        tab: "",
        undo: None,
        // Never show the topic itself: on ntfy.sh it works like a password.
        push: (!c.ntfy_topic.is_empty()).then(|| c.ntfy_url.clone()),
        digest_time: format!("{:02}:{:02}", c.digest_time.0, c.digest_time.1),
        backup: c
            .backup_dir
            .as_ref()
            .map(|d| format!("{} (keeps {} days)", d.display(), c.backup_keep_days)),
        backup_time: format!("{:02}:{:02}", c.backup_time.0, c.backup_time.1),
        lookup: c.lookup_enabled,
        version: env!("CARGO_PKG_VERSION"),
        lang: s.lang_setting(),
        langs: crate::i18n::LANGS.iter().map(|l| (l.1, l.2)).collect(),
        signed_in: !s.cfg.password.is_empty(),
    })
}

#[derive(Deserialize)]
struct LangForm {
    lang: String,
}

async fn set_lang(State(s): St, Form(f): Form<LangForm>) -> Response {
    if f.lang != "auto" && !crate::i18n::LANGS.iter().any(|l| l.1 == f.lang) {
        return back("/settings", "Unknown language");
    }
    if let Err(e) = s.store.set_setting("lang", &f.lang) {
        return back("/settings", format!("Not saved: {e}"));
    }
    if let Ok(mut l) = s.lang.write() {
        *l = f.lang;
    }
    Redirect::to("/settings").into_response()
}

async fn test_push(State(s): St) -> Response {
    match notify::send_digest(
        &s.store,
        &s.cfg.remind_days,
        s.notifier.as_ref(),
        db::today(),
        true,
    )
    .await
    {
        Ok(r) if r.sent => back("/settings", format!("Sent: {}", r.title)),
        Ok(r) => back(
            "/settings",
            format!("Not sent: {}", r.reason.unwrap_or_default()),
        ),
        Err(e) => back("/settings", format!("Not sent: {e}")),
    }
}

async fn backup_now(State(s): St) -> Response {
    let Some(dir) = s.cfg.backup_dir.clone() else {
        return back("/settings", "Backup is off (set VORRAT_BACKUP_DIR)");
    };
    let st = s.clone();
    let r = tokio::task::spawn_blocking(move || {
        crate::backup::run_backup(
            &st.store,
            &st.cfg.files_dir(),
            &dir,
            st.cfg.backup_keep_days,
            db::today(),
        )
    })
    .await;
    match r {
        Ok(Ok(p)) => back(
            "/settings",
            format!(
                "Backup written: {}",
                p.file_name().unwrap_or_default().to_string_lossy()
            ),
        ),
        Ok(Err(e)) => back("/settings", format!("Backup failed: {e}")),
        Err(e) => back("/settings", format!("Backup failed: {e}")),
    }
}

// -- static -------------------------------------------------------------------------

async fn static_file(Path(name): Path<String>) -> Response {
    let (body, ctype): (&'static [u8], &str) = match name.as_str() {
        "app.css" => (
            include_bytes!("../static/app.css"),
            "text/css; charset=utf-8",
        ),
        "manifest.webmanifest" => (
            include_bytes!("../static/manifest.webmanifest"),
            "application/manifest+json",
        ),
        "app.js" => (
            include_bytes!("../static/app.js"),
            "text/javascript; charset=utf-8",
        ),
        "print.js" => (
            include_bytes!("../static/print.js"),
            "text/javascript; charset=utf-8",
        ),
        "scan.js" => (
            include_bytes!("../static/scan.js"),
            "text/javascript; charset=utf-8",
        ),
        "zxing-0.23.0.min.js" => (
            include_bytes!("../static/vendor/zxing-0.23.0.min.js"),
            "text/javascript; charset=utf-8",
        ),
        "icon-180.png" => (include_bytes!("../static/icon-180.png"), "image/png"),
        "icon-192.png" => (include_bytes!("../static/icon-192.png"), "image/png"),
        "icon-512.png" => (include_bytes!("../static/icon-512.png"), "image/png"),
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

// -- JSON API -----------------------------------------------------------------------

#[derive(Deserialize)]
struct ApiItemsQ {
    q: Option<String>,
    kind: Option<String>,
    in_stock: Option<bool>,
    sort: Option<String>,
}

async fn api_items(State(s): St, Query(p): Query<ApiItemsQ>) -> Res<Json<Vec<db::Item>>> {
    Ok(Json(s.store.items(&ItemQuery {
        search: p.q.as_deref().unwrap_or(""),
        kind: p.kind.as_deref().unwrap_or(""),
        in_stock_only: p.in_stock.unwrap_or(false),
        sort: Sort::parse(p.sort.as_deref().unwrap_or("name")),
        location_id: None,
    })?))
}

async fn api_item(State(s): St, Path(id): Path<i64>) -> Res<Response> {
    let Some(item) = s.store.item(id)? else {
        return Ok(not_found());
    };
    let batches = rules::annotate(s.store.batches(Some(id))?, db::today());
    Ok(Json(serde_json::json!({ "item": item, "batches": batches })).into_response())
}

#[derive(Deserialize)]
struct WithinQ {
    within: Option<i64>,
}

async fn api_expiring(State(s): St, Query(p): Query<WithinQ>) -> Res<Json<Vec<Dated>>> {
    Ok(Json(rules::expiring(
        &s.store,
        db::today(),
        p.within.unwrap_or(14).clamp(0, 3650),
    )?))
}

async fn api_plan(State(s): St) -> Res<Json<rules::Plan>> {
    Ok(Json(rules::coverage(&s.store, db::today())?))
}

/// Preview of what today's reminder would say. Does not mark anything as sent.
async fn api_digest(State(s): St) -> Res<Json<serde_json::Value>> {
    let dg = rules::build_digest(&s.store, db::today(), &s.cfg.remind_days)?;
    Ok(Json(serde_json::json!({
        "empty": dg.is_empty(),
        "title": dg.title(),
        "body": dg.body(),
        "digest": dg,
        "currency": s.cfg.currency,
    })))
}

// -- JSON API: auth and writes --------------------------------------------------

/// With `VORRAT_API_TOKEN` set, `/api/` needs that bearer token. The pages don't
/// use the API, so they keep working without it.
async fn api_auth(State(s): St, req: Request, next: Next) -> Response {
    if req.uri().path().starts_with("/api/") && !s.cfg.api_token.is_empty() {
        let given = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or("");
        if !ct_eq(given.as_bytes(), s.cfg.api_token.as_bytes()) {
            return api_err(StatusCode::UNAUTHORIZED, "missing or wrong bearer token");
        }
    }
    next.run(req).await
}

/// Constant-time comparison, so the token can't be guessed byte by byte.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn api_err(code: StatusCode, msg: impl std::fmt::Display) -> Response {
    (code, Json(serde_json::json!({ "error": msg.to_string() }))).into_response()
}

/// Validation problems are the caller's fault (400); the store says why.
fn api_result<T: serde::Serialize>(code: StatusCode, r: anyhow::Result<T>) -> Response {
    match r {
        Ok(v) => (code, Json(v)).into_response(),
        Err(e) => api_err(StatusCode::BAD_REQUEST, format!("{e:#}")),
    }
}

async fn api_summary(State(s): St) -> Res<Json<serde_json::Value>> {
    let today = db::today();
    let snap = rules::Snapshot::load(&s.store, today)?;
    let soon = snap.expiring(7);
    let expired = soon
        .iter()
        .filter(|b| b.days_left.is_some_and(|d| d < 0))
        .count();
    let plan = rules::coverage_of(&s.store, &snap)?;
    Ok(Json(serde_json::json!({
        "items": snap.items.len(),
        "in_stock": snap.items.iter().filter(|i| i.total > 0.0).count(),
        "expired": expired,
        "expiring_7d": soon.len() - expired,
        "low": snap.low_stock().len(),
        "plan_ratio": plan.ratio,
        "plan_days": plan.days,
        "weakest_days": plan.weakest_days,
    })))
}

async fn api_barcode(State(s): St, Path(code): Path<String>) -> Res<Response> {
    let Some(item) = s.store.item_by_barcode(code.trim())? else {
        return Ok(api_err(StatusCode::NOT_FOUND, "unknown barcode"));
    };
    let batches = rules::annotate(s.store.batches(Some(item.id))?, db::today());
    Ok(Json(serde_json::json!({ "item": item, "batches": batches })).into_response())
}

async fn api_create_item(State(s): St, Json(mut f): Json<NewItem>) -> Response {
    // Pictures only come from the lookup; a client can't point at a file.
    f.image = None;
    if let Some(code) = f
        .barcode
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
    {
        match s.store.item_by_barcode(code) {
            Ok(Some(it)) => {
                return api_err(
                    StatusCode::CONFLICT,
                    format!("barcode already belongs to item {}", it.id),
                )
            }
            Ok(None) => {}
            Err(e) => return AppError(e).into_response(),
        }
    }
    api_result(
        StatusCode::CREATED,
        s.store
            .add_item(f)
            .map(|id| serde_json::json!({ "id": id })),
    )
}

#[derive(Deserialize)]
struct ApiBatch {
    qty: f64,
    expires_on: Option<NaiveDate>,
    location_id: Option<i64>,
    price: Option<f64>,
}

async fn api_add_batch(State(s): St, Path(id): Path<i64>, Json(b): Json<ApiBatch>) -> Response {
    match s.store.item(id) {
        Ok(Some(_)) => {}
        Ok(None) => return api_err(StatusCode::NOT_FOUND, "no such item"),
        Err(e) => return AppError(e).into_response(),
    }
    api_result(
        StatusCode::CREATED,
        s.store
            .add_batch(id, b.qty, b.expires_on, b.location_id, b.price)
            .map(|bid| serde_json::json!({ "id": bid })),
    )
}

#[derive(Deserialize)]
struct ApiConsume {
    qty: f64,
    batch_id: Option<i64>,
    #[serde(default)]
    waste: bool,
}

async fn api_consume(State(s): St, Path(id): Path<i64>, Json(c): Json<ApiConsume>) -> Response {
    match s.store.item(id) {
        Ok(Some(_)) => {}
        Ok(None) => return api_err(StatusCode::NOT_FOUND, "no such item"),
        Err(e) => return AppError(e).into_response(),
    }
    api_result(
        StatusCode::OK,
        s.store
            .consume(id, c.qty, c.batch_id, c.waste)
            .map(|used| serde_json::json!({ "used": used })),
    )
}

async fn api_shopping(State(s): St) -> Res<Json<Vec<db::ShoppingEntry>>> {
    Ok(Json(s.store.shopping()?))
}

#[derive(Deserialize)]
struct ApiShopping {
    label: Option<String>,
    item_id: Option<i64>,
    qty: Option<f64>,
}

async fn api_add_shopping(State(s): St, Json(e): Json<ApiShopping>) -> Response {
    let r = (|| {
        let label = match (
            e.label.as_deref().map(str::trim).filter(|l| !l.is_empty()),
            e.item_id,
        ) {
            (Some(l), _) => l.to_string(),
            (None, Some(id)) => match s.store.item(id)? {
                Some(it) => it.name,
                None => anyhow::bail!("no such item"),
            },
            (None, None) => anyhow::bail!("label or item_id is required"),
        };
        if e.qty.is_some_and(|q| !(q.is_finite() && q > 0.0)) {
            anyhow::bail!("quantity must be positive");
        }
        let id = s.store.add_shopping(&label, e.item_id, e.qty)?;
        Ok(serde_json::json!({ "id": id }))
    })();
    api_result(StatusCode::CREATED, r)
}

async fn api_suggestions(State(s): St) -> Res<Json<Vec<crate::smart::Suggestion>>> {
    Ok(Json(crate::smart::shopping_suggestions(
        &s.store,
        db::today(),
    )?))
}

// -- export / import ----------------------------------------------------------

fn download(ctype: &'static str, name: &str, body: Vec<u8>) -> Response {
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

async fn export_json(State(s): St) -> Res<Response> {
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let v = crate::transfer::export_json(&s.store, &now)?;
    Ok(download(
        "application/json",
        &format!("vorrat-{}.json", db::today()),
        serde_json::to_vec_pretty(&v)?,
    ))
}

async fn export_csv(State(s): St) -> Res<Response> {
    // A BOM so Excel reads the file as UTF-8 (umlauts).
    let mut body = "\u{feff}".as_bytes().to_vec();
    body.extend(crate::transfer::export_csv(&s.store)?.into_bytes());
    Ok(download(
        "text/csv; charset=utf-8",
        &format!("vorrat-stock-{}.csv", db::today()),
        body,
    ))
}

/// The whole database as one SQLite file: the most complete copy, and it
/// opens in any Vorrat with the same schema as is.
async fn export_db(State(s): St) -> Res<Response> {
    let tmp = s
        .cfg
        .data_dir
        .join(format!(".export-{}.db", crate::files::random_hex(8)));
    let st = s.clone();
    let path = tmp.clone();
    let bytes = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
        let r = st
            .store
            .backup_to(&path)
            .and_then(|_| Ok(std::fs::read(&path)?));
        let _ = std::fs::remove_file(&path);
        r
    })
    .await??;
    Ok(download(
        "application/vnd.sqlite3",
        &format!("vorrat-{}.db", db::today()),
        bytes,
    ))
}

const MAX_IMPORT: usize = 5 * 1024 * 1024;

async fn import_csv(State(s): St, mut mp: axum::extract::Multipart) -> Response {
    let mut text = None;
    loop {
        match mp.next_field().await {
            Ok(Some(f)) if f.name() == Some("file") => match f.bytes().await {
                Ok(b) => text = Some(b),
                Err(_) => return back("/settings", "Upload failed (max 5 MB)"),
            },
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(_) => return back("/settings", "Upload failed (max 5 MB)"),
        }
    }
    let Some(bytes) = text.filter(|b| !b.is_empty()) else {
        return back("/settings", "Choose a CSV file");
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return back("/settings", "Not a text file: save the CSV as UTF-8");
    };
    let text = text.to_string();
    let st = s.clone();
    let r =
        tokio::task::spawn_blocking(move || crate::transfer::import_csv(&st.store, &text)).await;
    match r {
        Ok(Ok(rep)) => back("/items", rep.summary()),
        Ok(Err(e)) => back("/settings", format!("Not imported: {e}")),
        Err(e) => back("/settings", format!("Not imported: {e}")),
    }
}

/// Served from the root so it may control every page. The version in the cache
/// name makes an upgrade replace the saved copies.
async fn service_worker() -> Response {
    static SW: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let body = SW.get_or_init(|| {
        include_str!("../static/sw.js").replace("__VERSION__", env!("CARGO_PKG_VERSION"))
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

// -- sign-in --------------------------------------------------------------------

/// With `VORRAT_PASSWORD` set, every page needs a session. API calls with the
/// bearer token (`VORRAT_API_TOKEN`) don't; `api_auth` checks those.
async fn sign_in(State(s): St, req: Request, next: Next) -> Response {
    if s.cfg.password.is_empty() {
        return next.run(req).await;
    }
    let path = req.uri().path();
    let bearer = req.headers().contains_key(header::AUTHORIZATION);
    if crate::auth::is_public(path)
        || (path.starts_with("/api/") && bearer && !s.cfg.api_token.is_empty())
    {
        return next.run(req).await;
    }
    let cookies = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok());
    if crate::auth::token_from(cookies).is_some_and(|t| s.sessions.valid(t)) {
        return next.run(req).await;
    }
    if req.method() == Method::GET && !path.starts_with("/api/") {
        let here = req
            .uri()
            .path_and_query()
            .map(|p| p.as_str())
            .unwrap_or("/");
        return Redirect::to(&format!("/login?next={}", url_encode(here))).into_response();
    }
    (StatusCode::UNAUTHORIZED, "sign in first").into_response()
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    next: String,
}

#[derive(Deserialize)]
struct LoginQ {
    next: Option<String>,
    msg: Option<String>,
}

async fn login_page(State(s): St, Query(q): Query<LoginQ>) -> Res<Response> {
    if s.cfg.password.is_empty() {
        return Ok(Redirect::to("/").into_response());
    }
    Ok(render(LoginT {
        msg: q.msg,
        tab: "login",
        undo: None,
        next: crate::auth::safe_next(q.next.as_deref()),
    })?
    .into_response())
}

#[derive(Deserialize)]
struct LoginForm {
    password: String,
    next: Option<String>,
}

async fn login(State(s): St, Form(f): Form<LoginForm>) -> Response {
    let next = crate::auth::safe_next(f.next.as_deref());
    let again = |msg: &str| {
        Redirect::to(&format!(
            "/login?next={}&msg={}",
            url_encode(&next),
            url_encode(msg)
        ))
        .into_response()
    };
    if s.cfg.password.is_empty() {
        return Redirect::to("/").into_response();
    }
    if s.sessions.locked_for().is_some() {
        return again("Too many attempts. Wait a minute.");
    }
    if !crate::auth::ct_eq(f.password.as_bytes(), s.cfg.password.as_bytes()) {
        s.sessions.failed();
        // Slow down guessing a little more.
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        tracing::warn!("wrong password at sign-in");
        return again("Wrong password");
    }
    s.sessions.succeeded();
    let Some(token) = s.sessions.create() else {
        return again("Could not start a session");
    };
    let secure = if s.cfg.public_url.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };
    let cookie = format!(
        "{}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{secure}",
        crate::auth::COOKIE,
        crate::auth::SESSION_DAYS * 86_400
    );
    ([(header::SET_COOKIE, cookie)], Redirect::to(&next)).into_response()
}

async fn logout(State(s): St, headers: HeaderMap) -> Response {
    let cookies = headers.get(header::COOKIE).and_then(|v| v.to_str().ok());
    if let Some(t) = crate::auth::token_from(cookies) {
        s.sessions.end(t);
    }
    (
        [(
            header::SET_COOKIE,
            format!(
                "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
                crate::auth::COOKIE
            ),
        )],
        Redirect::to("/login"),
    )
        .into_response()
}

// -- power cut ------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "power_cut.html")]
struct PowerCutT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    fridge: Vec<BatchRow>,
    freezer: Vec<BatchRow>,
    water_days: Option<String>,
    people: i64,
}

async fn power_cut_page(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    let today = db::today();
    let pc = rules::power_cut(&s.store, today)?;
    let plan = rules::coverage(&s.store, today)?;
    let water_days = plan
        .lines
        .iter()
        .find(|c| c.target.category == "water")
        .and_then(|c| c.days)
        .map(fmt_qty);
    render(PowerCutT {
        msg: f.msg,
        tab: "plan",
        undo: f.undo,
        fridge: pc.fridge.iter().map(BatchRow::from).collect(),
        freezer: pc.freezer.iter().map(BatchRow::from).collect(),
        water_days,
        people: plan.people,
    })
}
