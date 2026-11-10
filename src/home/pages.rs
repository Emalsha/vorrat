//! The home page.

use askama::Template;
use axum::extract::{Query, State};
use axum::response::Html;

use crate::db;
use crate::i18n::{tr, trf};
use crate::plan::pages::{gap_labels, ready_days};
use crate::stock::pages::{BatchRow, ItemRow};
use crate::stock::rules::fmt_qty;
use crate::web::respond::{render, Flash, Res};
use crate::web::view::{bar_class, day_cells, filters, money, pct};
use crate::web::St;

#[derive(Template)]
#[template(path = "home/home.html")]
struct HomeT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    expired: Vec<BatchRow>,
    use_first: Vec<BatchRow>,
    low: Vec<ItemRow>,
    attention: usize,
    plan_pct: u32,
    plan_bar: &'static str,
    people: i64,
    pets: i64,
    ready_days: Option<String>,
    ready_of: String,
    ready_cells: Vec<bool>,
    gaps: Vec<String>,
    recent: Vec<String>,
    month_note: Option<String>,
}

/// The home page.
pub(super) async fn home(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    let today = db::today();
    let snap = s.store.snapshot(today)?;
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
    let (expired, use_first): (Vec<BatchRow>, Vec<BatchRow>) = snap
        .expiring(14)
        .iter()
        .map(BatchRow::from)
        .partition(|b| b.status == "expired");
    let low: Vec<ItemRow> = snap.low_stock().iter().map(ItemRow::from).collect();
    let plan = s.store.plan_for(&snap)?;
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
        attention: expired.len() + use_first.len() + low.len(),
        expired,
        use_first,
        low,
        plan_pct: pct(plan.ratio),
        plan_bar: bar_class(plan.ratio),
        people: plan.people,
        pets: plan.pets,
        ready_days: ready_days(&plan),
        ready_of: trf("of {0} days", &[&plan.days]),
        ready_cells: day_cells(plan.weakest_days, plan.days),
        gaps: gap_labels(&plan),
        recent,
        month_note,
    })
}
