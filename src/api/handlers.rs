//! Handlers of the JSON API.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::NaiveDate;
use serde::Deserialize;

use crate::plan::rules::Plan;
use crate::shopping::queries::ShoppingEntry;
use crate::stock::queries::{Item, ItemQuery, NewItem, Sort};
use crate::stock::rules::{Dated, Snapshot};
use crate::web::respond::{not_found, AppError, Res};
use crate::web::St;
use crate::{db, jobs, plan, stock};

#[derive(Deserialize)]
pub(super) struct ApiItemsQ {
    q: Option<String>,
    kind: Option<String>,
    in_stock: Option<bool>,
    sort: Option<String>,
}

pub(super) async fn api_items(State(s): St, Query(p): Query<ApiItemsQ>) -> Res<Json<Vec<Item>>> {
    Ok(Json(s.store.items(&ItemQuery {
        search: p.q.as_deref().unwrap_or(""),
        kind: p.kind.as_deref().unwrap_or(""),
        in_stock_only: p.in_stock.unwrap_or(false),
        sort: Sort::parse(p.sort.as_deref().unwrap_or("name")),
        location_id: None,
    })?))
}

pub(super) async fn api_item(State(s): St, Path(id): Path<i64>) -> Res<Response> {
    let Some(item) = s.store.item(id)? else {
        return Ok(not_found());
    };
    let batches = stock::rules::annotate(s.store.batches(Some(id))?, db::today());
    Ok(Json(serde_json::json!({ "item": item, "batches": batches })).into_response())
}

#[derive(Deserialize)]
pub(super) struct WithinQ {
    within: Option<i64>,
}

pub(super) async fn api_expiring(State(s): St, Query(p): Query<WithinQ>) -> Res<Json<Vec<Dated>>> {
    Ok(Json(stock::rules::expiring(
        &s.store,
        db::today(),
        p.within.unwrap_or(14).clamp(0, 3650),
    )?))
}

pub(super) async fn api_plan(State(s): St) -> Res<Json<Plan>> {
    Ok(Json(plan::rules::coverage(&s.store, db::today())?))
}

/// Preview of what today's reminder would say. Does not mark anything as sent.
pub(super) async fn api_digest(State(s): St) -> Res<Json<serde_json::Value>> {
    let dg = jobs::digest::build_digest(&s.store, db::today(), &s.cfg.remind_days)?;
    Ok(Json(serde_json::json!({
        "empty": dg.is_empty(),
        "title": dg.title(),
        "body": dg.body(),
        "digest": dg,
        "currency": s.cfg.currency,
    })))
}

pub(super) async fn api_summary(State(s): St) -> Res<Json<serde_json::Value>> {
    let today = db::today();
    let snap = Snapshot::load(&s.store, today)?;
    let soon = snap.expiring(7);
    let expired = soon
        .iter()
        .filter(|b| b.days_left.is_some_and(|d| d < 0))
        .count();
    let plan = plan::rules::coverage_of(&s.store, &snap)?;
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

pub(super) async fn api_barcode(State(s): St, Path(code): Path<String>) -> Res<Response> {
    let Some(item) = s.store.item_by_barcode(code.trim())? else {
        return Ok(api_err(StatusCode::NOT_FOUND, "unknown barcode"));
    };
    let batches = stock::rules::annotate(s.store.batches(Some(item.id))?, db::today());
    Ok(Json(serde_json::json!({ "item": item, "batches": batches })).into_response())
}

/// Creates an item from JSON. Pictures only come from the lookup, so a client
/// can't point at a file.
pub(super) async fn api_create_item(State(s): St, Json(mut f): Json<NewItem>) -> Response {
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
pub(super) struct ApiBatch {
    qty: f64,
    expires_on: Option<NaiveDate>,
    location_id: Option<i64>,
    price: Option<f64>,
}

pub(super) async fn api_add_batch(
    State(s): St,
    Path(id): Path<i64>,
    Json(b): Json<ApiBatch>,
) -> Response {
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
pub(super) struct ApiConsume {
    qty: f64,
    batch_id: Option<i64>,
    #[serde(default)]
    waste: bool,
}

pub(super) async fn api_consume(
    State(s): St,
    Path(id): Path<i64>,
    Json(c): Json<ApiConsume>,
) -> Response {
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

pub(super) async fn api_shopping(State(s): St) -> Res<Json<Vec<ShoppingEntry>>> {
    Ok(Json(s.store.shopping()?))
}

#[derive(Deserialize)]
pub(super) struct ApiShopping {
    label: Option<String>,
    item_id: Option<i64>,
    qty: Option<f64>,
}

pub(super) async fn api_add_shopping(State(s): St, Json(e): Json<ApiShopping>) -> Response {
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

pub(super) async fn api_suggestions(State(s): St) -> Res<Json<Vec<crate::smart::Suggestion>>> {
    Ok(Json(crate::smart::shopping_suggestions(
        &s.store,
        db::today(),
    )?))
}

pub(super) fn api_err(code: StatusCode, msg: impl std::fmt::Display) -> Response {
    (code, Json(serde_json::json!({ "error": msg.to_string() }))).into_response()
}

/// Validation problems are the caller's fault (400); the store says why.
pub(super) fn api_result<T: serde::Serialize>(code: StatusCode, r: anyhow::Result<T>) -> Response {
    match r {
        Ok(v) => (code, Json(v)).into_response(),
        Err(e) => api_err(StatusCode::BAD_REQUEST, format!("{e:#}")),
    }
}
