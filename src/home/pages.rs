//! The home page.

use askama::Template;
use axum::extract::{Query, State};
use axum::response::Html;

use crate::i18n::{tr, trf};
use crate::plan::pages::{gap_labels, ready_days};
use crate::stock::pages::{BatchRow, ItemRow};
use crate::stock::rules::{fmt_qty, Snapshot};
use crate::web::respond::{render, Flash, Res};
use crate::web::view::{bar_class, filters, money, pct};
use crate::web::St;
use crate::{db, plan};

#[derive(Template)]
#[template(path = "home/home.html")]
struct HomeT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    expiring: Vec<BatchRow>,
    low: Vec<ItemRow>,
    attention: usize,
    plan_pct: u32,
    plan_bar: &'static str,
    people: i64,
    pets: i64,
    ready_days: Option<String>,
    ready_of: String,
    gaps: Vec<String>,
    recent: Vec<String>,
    month_note: Option<String>,
}

/// The home page.
pub(super) async fn home(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    let today = db::today();
    let snap = Snapshot::load(&s.store, today)?;
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
    let expiring: Vec<BatchRow> = snap.expiring(14).iter().map(BatchRow::from).collect();
    let low: Vec<ItemRow> = snap.low_stock().iter().map(ItemRow::from).collect();
    let plan = plan::rules::coverage_of(&s.store, &snap)?;
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
        attention: expiring.len() + low.len(),
        expiring,
        low,
        plan_pct: pct(plan.ratio),
        plan_bar: bar_class(plan.ratio),
        people: plan.people,
        pets: plan.pets,
        ready_days: ready_days(&plan),
        ready_of: trf("of {0} days", &[&plan.days]),
        gaps: gap_labels(&plan),
        recent,
        month_note,
    })
}
