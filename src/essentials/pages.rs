//! The essentials page: medicine and checklists.

use askama::Template;
use axum::extract::{Form, Path, Query, State};
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;

use crate::db;
use crate::essentials::queries::MED_FORMS;
use crate::essentials::rules;
use crate::essentials::rules::CheckState;
use crate::i18n::tr;
use crate::stock::queries::{Item, NewItem};
use crate::stock::rules::fmt_qty;
use crate::web::form::{opt_date, opt_f64, opt_i64};
use crate::web::respond::{back, done, not_found, render, Flash, Res};
use crate::web::view::{bar_class, filters, hdate, pct};
use crate::web::St;

/// The medicine and essentials page.
///
/// - `groups`: one group per person or pet who takes something, then "not assigned".
/// - `undosed`: medicine-category items without a daily dose.
#[derive(Template)]
#[template(path = "essentials/essentials.html")]
struct EssentialsT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    plan_days: i64,
    groups: Vec<MedGroup>,
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

/// A medicine as the essentials page shows it.
///
/// - `detail`: "400 mg · tablet", translated form.
/// - `stock`: "28 pcs · 2 a day"
/// - `renew`: "Renew prescription by 12 Dec", with whether it is soon or overdue.
struct MedRow {
    id: i64,
    name: String,
    detail: String,
    stock: String,
    days: String,
    pct: u32,
    bar: &'static str,
    renew: Option<(String, bool)>,
    expiry: Option<String>,
}

/// A checklist entry as the essentials page shows it.
///
/// - `count`: "1 / 3" for per-person entries; empty otherwise.
/// - `candidates`: items that could stand for it: same category, not linked yet.
struct CheckRow {
    key: &'static str,
    label: &'static str,
    ok: bool,
    expired: bool,
    count: String,
    items: Vec<(i64, String)>,
    named_by: String,
    candidates: Vec<(i64, String)>,
}

fn check_rows(states: Vec<CheckState>, items: &[Item]) -> (Vec<CheckRow>, usize) {
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

pub(super) async fn essentials_page(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    use crate::essentials::rules as es;
    let today = db::today();
    let snap = s.store.snapshot(today)?;
    let members = s.store.members()?;
    let labels = es::member_labels(&members);
    let plan_days = s.store.setting_int("days", 14).max(1);
    let meds = es::medicines(s.store.doses()?, &snap);
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
        forms: MED_FORMS,
    })
}

#[derive(Deserialize)]
pub(super) struct DoseForm {
    name: Option<String>,
    unit: Option<String>,
    member_id: Option<String>,
    form: Option<String>,
    strength: Option<String>,
    per_day: Option<String>,
    renew_on: Option<String>,
    delete: Option<String>,
}

/// Adds a medicine, then opens its item page to enter how much is in the
/// cupboard and until when it keeps.
pub(super) async fn add_medicine(State(s): St, Form(f): Form<DoseForm>) -> Response {
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
        Ok(id) => back(
            &format!("/items/{id}"),
            "Medicine added. Now add the stock you have.",
        )
        .into_response(),
        Err(e) => back("/essentials", format!("Not saved: {e}")),
    }
}

pub(super) async fn set_dose(
    State(s): St,
    Path(id): Path<i64>,
    Form(f): Form<DoseForm>,
) -> Response {
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

/// A button on a checklist entry or suggestion.
///
/// - `action`: `add`, `shop`, `link` (with `item_id`), or `dismiss` for a suggestion.
#[derive(Deserialize)]
pub(super) struct CheckForm {
    action: String,
    item_id: Option<String>,
}

/// Acts on a checklist entry or suggestion. Suggestions live on the plan page,
/// the checklists on their own page. "Not needed" is for good; it comes back
/// only if the data is reset.
pub(super) async fn check_action(
    State(s): St,
    Path(key): Path<String>,
    Form(f): Form<CheckForm>,
) -> Response {
    let Some(c) = rules::find(&key) else {
        return not_found();
    };
    let label = tr(c.label);
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
        "dismiss" if suggestion => done(
            page,
            s.store
                .dismiss(&rules::dismiss_key(c.key), "9999-12-31")
                .map(|_| "Hidden".into()),
        ),
        _ => not_found(),
    }
}

pub(super) async fn clear_preset(State(s): St, Path(id): Path<i64>) -> Response {
    done(
        &format!("/items/{id}"),
        s.store.set_preset(id, None).map(|_| "Saved".into()),
    )
}
