//! Item list, item page, adding and using stock, and the expiry calendar.

use askama::Template;
use axum::extract::{Form, Path, Query, State};
use axum::http::header;
use axum::response::{Html, IntoResponse, Response};
use chrono::NaiveDate;
use serde::Deserialize;

use super::rules;
use crate::essentials::queries::MED_FORMS;
use crate::i18n::{tr, trf};
use crate::locations::queries::Location;
use crate::stock::queries::{Attachment, Item, ItemQuery, NewItem, Sort, CATEGORIES, KINDS, UNITS};
use crate::stock::rules::{fmt_qty, Dated, Status};
use crate::web::form::{opt_date, opt_f64, opt_i64};
use crate::web::respond::{back, back_undo, done, not_found, render, safe_back, Flash, Res};
use crate::web::view::{filters, hdate, initial, money, when};
use crate::web::St;
use crate::{db, essentials};

pub(crate) struct BatchRow {
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

pub(crate) struct ItemRow {
    pub image: Option<String>,
    pub id: i64,
    pub name: String,
    pub initial: String,
    pub sub: String,
    pub status: &'static str,
    pub when: String,
}

impl From<&Item> for ItemRow {
    fn from(i: &Item) -> Self {
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
            status: {
                let days = rules::days_left(i.next_expiry.as_deref(), db::today());
                match Status::of(days, 14) {
                    Status::Expired => "expired",
                    _ if i.is_low() => "low",
                    Status::Urgent => "urgent",
                    Status::Soon => "soon",
                    _ => "",
                }
            },
            when: when(rules::days_left(i.next_expiry.as_deref(), db::today())),
        }
    }
}

#[derive(Template)]
#[template(path = "stock/items.html")]
struct ItemsT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    q: String,
    kind: String,
    kinds: &'static [(&'static str, &'static str)],
    items: Vec<ItemRow>,
    shown: String,
    low: bool,
    sort: &'static str,
}

#[derive(Deserialize)]
pub(super) struct ItemsQ {
    q: Option<String>,
    kind: Option<String>,
    sort: Option<String>,
    low: Option<String>,
    msg: Option<String>,
}

pub(super) async fn items_page(State(s): St, Query(p): Query<ItemsQ>) -> Res<Html<String>> {
    let q = p.q.unwrap_or_default();
    let kind = p.kind.unwrap_or_default();
    let low = p.low.is_some_and(|v| v == "1");
    let sort = match p.sort.as_deref() {
        Some("expiry") => "expiry",
        Some("recent") => "recent",
        _ => "name",
    };
    let items = s.store.items(&ItemQuery {
        search: &q,
        kind: &kind,
        sort: Sort::parse(sort),
        ..Default::default()
    })?;
    let items: Vec<ItemRow> = items
        .iter()
        .filter(|i| !low || i.is_low())
        .map(ItemRow::from)
        .collect();
    render(ItemsT {
        msg: p.msg,
        tab: "items",
        undo: None,
        shown: trf("Shown: {0}", &[&items.len()]),
        items,
        low,
        sort,
        q,
        kind,
        kinds: KIND_FILTERS,
    })
}

/// The kind filters on the stock page, with their plural labels.
const KIND_FILTERS: &[(&str, &str)] = &[
    ("food", "Food"),
    ("supply", "Supplies"),
    ("document", "Documents"),
    ("thing", "Things"),
];

/// The item page.
///
/// - `back_to`: where the batch buttons return to (used by `_batch_row.html`).
/// - `show_dose`: medicines: the daily dose form is shown for the medicine category or
///   when a dose is set.
/// - `members`: (id, label, chosen) for the "for" select.
/// - `preset`: the checklist entry this item counts for.
#[derive(Template)]
#[template(path = "stock/item.html")]
struct ItemT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    item: Item,
    total: String,
    min_qty: String,
    batches: Vec<BatchRow>,
    history: Vec<String>,
    locations: Vec<Location>,
    attachments: Vec<Attachment>,
    prices: Vec<String>,
    extra_barcodes: Vec<String>,
    insights: Vec<String>,
    restock_qty: String,
    expiry_hint: Option<(String, String)>,
    back_to: String,
    show_dose: bool,
    has_dose: bool,
    dose: DoseView,
    members: Vec<(i64, String, bool)>,
    forms: &'static [&'static str],
    preset: Option<&'static str>,
}

/// A daily dose as the item page shows it.
///
/// - `days`: "lasts 14 days"
#[derive(Default)]
struct DoseView {
    member_id: Option<i64>,
    form: String,
    strength: String,
    per_day: String,
    renew_on: String,
    days: String,
}

pub(super) async fn item_page(
    State(s): St,
    Path(id): Path<i64>,
    Query(f): Query<Flash>,
) -> Res<Response> {
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
        let labels = essentials::rules::member_labels(&ms);
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
        forms: MED_FORMS,
        preset: item
            .preset
            .as_deref()
            .and_then(essentials::rules::find)
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
#[template(path = "stock/add.html")]
struct AddT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    p: Prefill,
    similar: Vec<ItemRow>,
    kinds: &'static [&'static str],
    units: &'static [&'static str],
    categories: &'static [&'static str],
    locations: Vec<Location>,
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

pub(super) async fn add_page(State(s): St, Query(mut p): Query<Prefill>) -> Res<Html<String>> {
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
pub(super) struct AddForm {
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

/// Adds stock. A known barcode or a name that already exists adds to that item
/// instead of making a duplicate.
pub(super) async fn add_submit(State(s): St, Form(f): Form<AddForm>) -> Response {
    let r = (|| -> anyhow::Result<(i64, String)> {
        let qty = opt_f64(&f.qty)?.unwrap_or(1.0);
        let expires = opt_date(&f.expires_on)?;
        let loc = opt_i64(&f.location_id);
        let price = opt_f64(&f.price)?;
        let existing = match f
            .barcode
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
        {
            Some(code) => s.store.item_by_barcode(code)?,
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
pub(super) struct BatchForm {
    qty: Option<String>,
    expires_on: Option<String>,
    location_id: Option<String>,
    price: Option<String>,
}

pub(super) async fn add_batch(
    State(s): St,
    Path(id): Path<i64>,
    Form(f): Form<BatchForm>,
) -> Response {
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
pub(super) struct UseForm {
    qty: Option<String>,
    batch_id: Option<String>,
    waste: Option<String>,
    back: Option<String>,
}

pub(super) async fn use_item(
    State(s): St,
    Path(id): Path<i64>,
    Form(f): Form<UseForm>,
) -> Response {
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

pub(super) async fn undo(State(s): St, Path(token): Path<String>) -> Response {
    match s.store.undo(&token) {
        Ok((Some(id), m)) => back(&format!("/items/{id}"), m),
        Ok((None, m)) => back("/", m),
        Err(e) => back("/", format!("Not undone: {e}")),
    }
}

pub(super) async fn open_batch(State(s): St, Path(id): Path<i64>) -> Response {
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
pub(super) struct MinForm {
    min_qty: Option<String>,
}

pub(super) async fn set_minimum(
    State(s): St,
    Path(id): Path<i64>,
    Form(f): Form<MinForm>,
) -> Response {
    done(
        &format!("/items/{id}"),
        (|| {
            s.store
                .set_min_qty(id, opt_f64(&f.min_qty)?.unwrap_or(0.0))?;
            Ok("Minimum saved".into())
        })(),
    )
}

pub(super) async fn delete_item(State(s): St, Path(id): Path<i64>) -> Response {
    let files = s.store.item_files(id).unwrap_or_default();
    let r = s.store.delete_item(id);
    if r.is_ok() {
        for f in files.iter().filter(|f| crate::files::is_safe_name(f)) {
            let _ = std::fs::remove_file(s.cfg.files_dir().join(f));
        }
    }
    done("/items", r.map(|_| "Deleted".into()))
}

#[derive(Template)]
#[template(path = "stock/item_form.html")]
struct ItemFormT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    item: Item,
    min_qty: String,
    size: String,
    open_days: String,
    kinds: &'static [&'static str],
    units: &'static [&'static str],
    size_units: &'static [&'static str],
    categories: &'static [&'static str],
    locations: Vec<Location>,
}

/// The edit form. Pack sizes use every unit but "pcs".
pub(super) async fn edit_item_page(
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
        size_units: &UNITS[1..],
        categories: CATEGORIES,
        locations: s.store.locations()?,
    })?
    .into_response())
}

#[derive(Deserialize)]
pub(super) struct EditForm {
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
    no_cook: Option<String>,
}

pub(super) async fn edit_item(
    State(s): St,
    Path(id): Path<i64>,
    Form(f): Form<EditForm>,
) -> Response {
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
                no_cook: f.no_cook.is_some(),
                ..Default::default()
            },
        )
    })();
    match r {
        Ok(()) => back(&format!("/items/{id}"), "Saved"),
        Err(e) => back(&format!("/items/{id}/edit"), format!("Not saved: {e}")),
    }
}

#[derive(Deserialize)]
pub(super) struct BackForm {
    back: Option<String>,
}

pub(super) async fn delete_batch(
    State(s): St,
    Path(id): Path<i64>,
    Form(f): Form<BackForm>,
) -> Response {
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

/// Expiry and renewal dates as an iCal feed, for the phone's calendar.
pub(super) async fn calendar(State(s): St) -> Res<Response> {
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

fn item_unit(i: &Item) -> &str {
    if i.unit == "pcs" {
        "piece"
    } else {
        &i.unit
    }
}
