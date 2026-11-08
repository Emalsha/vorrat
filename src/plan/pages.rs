//! The plan page, its printable checklist and the power-cut view.

use askama::Template;
use axum::extract::{Form, Path, Query, State};
use axum::response::{Html, Response};
use serde::Deserialize;

use crate::i18n::{tr, trf};
use crate::plan::rules;
use crate::plan::rules::{Coverage, Plan};
use crate::stock::pages::BatchRow;
use crate::stock::queries::{CATEGORIES, UNITS};
use crate::stock::rules::{fmt_qty, Snapshot};
use crate::web::form::{opt_f64, opt_i64};
use crate::web::respond::{done, render, Flash, Res};
use crate::web::view::{bar_class, filters, pct};
use crate::web::St;
use crate::{db, stock};

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
    pub contributors: String,
    pub status: &'static str,
    pub status_text: String,
}

impl From<&Coverage> for PlanLine {
    fn from(c: &Coverage) -> Self {
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
            contributors,
            status: match bar_class(c.ratio) {
                _ if c.missing <= 0.0 => "ok",
                "crit" => "crit",
                _ => "warn",
            },
            status_text: if c.missing <= 0.0 {
                tr("Covered")
            } else {
                format!(
                    "{} {} {}",
                    tr("Missing"),
                    fmt_qty(c.missing),
                    tr(&c.target.unit)
                )
            },
        }
    }
}

/// Days the weakest essential lasts, at most the days planned for: the big
/// number on Home and Plan.
pub(crate) fn ready_days(plan: &Plan) -> Option<String> {
    plan.weakest_days
        .map(|w| fmt_qty(w.clamp(0.0, plan.days as f64).floor()))
}

/// The first three targets not yet covered, then "+2 more". Labels contain
/// commas, so the template joins them with "; ".
pub(crate) fn gap_labels(plan: &Plan) -> Vec<String> {
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
}

/// The plan page.
///
/// - `first_aid`: (have, of) for the first-aid kit and the household essentials.
#[derive(Template)]
#[template(path = "plan/plan.html")]
struct PlanT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    people: i64,
    pets: i64,
    days: i64,
    plan_pct: u32,
    plan_bar: &'static str,
    ready_days: Option<String>,
    ready_of: String,
    gaps: Vec<String>,
    lines: Vec<PlanLine>,
    has_gaps: bool,
    categories: &'static [&'static str],
    units: &'static [&'static str],
    presets: &'static [(i64, &'static str)],
    modes: &'static [(&'static str, &'static str)],
    medicine_days: Option<String>,
    suggestions: Vec<SuggestRow>,
    first_aid: (usize, usize),
    essentials: (usize, usize),
}

/// A suggestion as the plan page shows it.
///
/// - `who`: the people and pets it is for.
struct SuggestRow {
    key: &'static str,
    label: &'static str,
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

pub(super) async fn plan_page(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    use crate::essentials::rules as es;
    let snap = Snapshot::load(&s.store, db::today())?;
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
        plan_pct: pct(plan.ratio),
        plan_bar: bar_class(plan.ratio),
        ready_days: ready_days(&plan),
        ready_of: trf("of {0} days", &[&plan.days]),
        gaps: gap_labels(&plan),
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
#[template(path = "plan/plan_print.html")]
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

pub(super) async fn plan_print(State(s): St) -> Res<Html<String>> {
    let today = db::today();
    let plan = rules::coverage(&s.store, today)?;
    let unit = |v: f64, u: &str| format!("{} {u}", fmt_qty(v));
    let soon = stock::rules::expiring(&s.store, today, plan.days)?
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
pub(super) struct PlanSettings {
    days: Option<String>,
}

/// The number of days. Who the plan is for comes from the household page.
pub(super) async fn plan_settings(State(s): St, Form(f): Form<PlanSettings>) -> Response {
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

#[derive(Deserialize)]
pub(super) struct TargetForm {
    label: String,
    category: String,
    mode: String,
    amount: Option<String>,
    unit: String,
    delete: Option<String>,
}

pub(super) async fn add_target(State(s): St, Form(f): Form<TargetForm>) -> Response {
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

pub(super) async fn edit_target(
    State(s): St,
    Path(id): Path<i64>,
    Form(f): Form<TargetForm>,
) -> Response {
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

pub(super) async fn plan_to_shopping(State(s): St) -> Response {
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

#[derive(Template)]
#[template(path = "plan/power_cut.html")]
struct PowerCutT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    fridge: Vec<BatchRow>,
    freezer: Vec<BatchRow>,
    water_days: Option<String>,
    people: i64,
}

pub(super) async fn power_cut_page(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
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
