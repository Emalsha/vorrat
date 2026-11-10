//! The country page: pick a country, see what its authority recommends, and
//! use those numbers in the plan.

use askama::Template;
use axum::extract::{Form, Query, State};
use axum::response::{Html, Response};
use serde::Deserialize;

use crate::country::rules::{self, Changes, Country, Status, COUNTRIES};
use crate::i18n::{tr, trf};
use crate::stock::rules::fmt_qty;
use crate::web::respond::{done, render, Flash, Res};
use crate::web::view::filters;
use crate::web::St;

#[derive(Template)]
#[template(path = "country/country.html")]
struct CountryT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    countries: Vec<(&'static str, String)>,
    chosen: Option<Chosen>,
}

/// The chosen country as the page shows it.
///
/// - `numbers`: false when the guide is not verified, so only the link is shown.
/// - `days`, `water`, `cash`: the recommendation, as text.
/// - `changes`: what using the numbers would change, one line each.
/// - `iodine`: what the guide says about iodine tablets, English.
struct Chosen {
    code: &'static str,
    name: String,
    authority: &'static str,
    url: &'static str,
    numbers: bool,
    days: Option<String>,
    water: Option<String>,
    cash: Option<String>,
    iodine: Option<&'static str>,
    changes: Vec<String>,
}

/// The text lines for a set of changes.
fn change_lines(ch: &Changes) -> Vec<String> {
    let mut out = Vec::new();
    if let Some((from, to)) = ch.days {
        out.push(trf("Plan length: {0} days instead of {1}", &[&to, &from]));
    }
    if let Some((_, from, to)) = ch.water {
        out.push(match from {
            Some(f) => trf(
                "Drinking water: {0} L per person per day instead of {1} L",
                &[&fmt_qty(to), &fmt_qty(f)],
            ),
            None => trf(
                "Drinking water: a new target of {0} L per person per day",
                &[&fmt_qty(to)],
            ),
        });
    }
    for (mode, amount, unit) in &ch.cash {
        out.push(match *mode {
            "per_adult" => trf("Cash: {0} {1} per adult", &[&fmt_qty(*amount), unit]),
            _ => trf("Cash: {0} {1} per child", &[&fmt_qty(*amount), unit]),
        });
    }
    out
}

fn chosen(c: &'static Country, days: i64, targets: &[crate::plan::queries::Target]) -> Chosen {
    Chosen {
        code: c.code,
        name: tr(c.name),
        authority: c.authority,
        url: c.url,
        numbers: c.status == Status::Verified,
        days: c.days.map(|d| trf("{0} days", &[&d])),
        water: c
            .water
            .map(|w| trf("{0} L per person per day", &[&fmt_qty(w)])),
        cash: (!c.cash.is_empty()).then(|| {
            c.cash
                .iter()
                .map(|(mode, amount, unit)| match *mode {
                    "per_adult" => trf("{0} {1} per adult", &[&fmt_qty(*amount), unit]),
                    _ => trf("{0} {1} per child", &[&fmt_qty(*amount), unit]),
                })
                .collect::<Vec<_>>()
                .join(", ")
        }),
        iodine: rules::iodine(c.code),
        changes: change_lines(&rules::changes(c, days, targets)),
    }
}

pub(super) async fn country_page(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    let code = s.store.setting("country")?.unwrap_or_default();
    let days = s.store.setting_int("days", 14).max(1);
    let targets = s.store.targets()?;
    let mut countries: Vec<(&'static str, String)> =
        COUNTRIES.iter().map(|c| (c.code, tr(c.name))).collect();
    countries.sort_by(|a, b| (a.0 != "EU", &a.1).cmp(&(b.0 != "EU", &b.1)));
    render(CountryT {
        msg: f.msg,
        tab: "plan",
        undo: None,
        countries,
        chosen: rules::find(&code).map(|c| chosen(c, days, &targets)),
    })
}

#[derive(Deserialize)]
pub(super) struct ChooseForm {
    country: String,
}

/// Saves the choice. Nothing in the plan changes until the household uses the numbers.
pub(super) async fn choose(State(s): St, Form(f): Form<ChooseForm>) -> Response {
    done(
        "/country",
        (|| {
            let c = rules::find(&f.country).ok_or_else(|| anyhow::anyhow!("unknown country"))?;
            s.store.set_setting("country", c.code)?;
            Ok("Saved".into())
        })(),
    )
}

/// Uses the chosen country's plan length, drinking-water amount and cash amounts.
pub(super) async fn apply(State(s): St) -> Response {
    done(
        "/plan",
        (|| {
            let code = s.store.setting("country")?.unwrap_or_default();
            let c = rules::find(&code).ok_or_else(|| anyhow::anyhow!("choose a country first"))?;
            let days = s.store.setting_int("days", 14).max(1);
            let targets = s.store.targets()?;
            let ch = rules::changes(c, days, &targets);
            if let Some((_, to)) = ch.days {
                s.store.set_setting("days", &to.to_string())?;
            }
            if let Some((id, _, to)) = ch.water {
                match id.and_then(|id| targets.iter().find(|t| t.id == id)) {
                    Some(t) => {
                        s.store
                            .update_target(t.id, &t.label, &t.category, &t.mode, to, &t.unit)?
                    }
                    None => {
                        s.store.add_target(
                            "Drinking water & drinks",
                            "water",
                            "per_person_day",
                            to,
                            "L",
                        )?;
                    }
                }
            }
            for (mode, amount, unit) in &ch.cash {
                s.store
                    .add_target("Cash in small notes", "cash", mode, *amount, unit)?;
            }
            Ok(if ch.is_empty() {
                "Nothing to change".into()
            } else {
                "The plan now follows your country's guide".into()
            })
        })(),
    )
}
