//! Expiry status, low stock and quantities. Kept apart from the pages so the same logic serves the pages, the API and the jobs.

use chrono::NaiveDate;
use serde::Serialize;

use crate::stock::queries::{Batch, Item, CURRENCIES};

/// Measurement dimension. Quantities only add up within one dimension, and
/// money only within one currency (`Money("EUR")`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Dim {
    Mass,
    Volume,
    Count,
    Money(&'static str),
}

/// Convert to the base unit of its dimension: g, ml, pcs or the currency itself.
pub fn to_base(value: f64, unit: &str) -> (Dim, f64) {
    match unit {
        "g" => (Dim::Mass, value),
        "kg" => (Dim::Mass, value * 1000.0),
        "ml" => (Dim::Volume, value),
        "L" => (Dim::Volume, value * 1000.0),
        _ => match CURRENCIES.iter().find(|c| **c == unit) {
            Some(c) => (Dim::Money(c), value),
            None => (Dim::Count, value),
        },
    }
}

pub fn days_left(expires_on: Option<&str>, today: NaiveDate) -> Option<i64> {
    let d = NaiveDate::parse_from_str(expires_on?, "%Y-%m-%d").ok()?;
    Some((d - today).num_days())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    None,
    Expired,
    Urgent,
    Soon,
    Ok,
}

impl Status {
    pub fn of(days: Option<i64>, soon: i64) -> Self {
        match days {
            None => Status::None,
            Some(d) if d < 0 => Status::Expired,
            Some(d) if d <= 7 => Status::Urgent,
            Some(d) if d <= soon => Status::Soon,
            Some(_) => Status::Ok,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Status::None => "none",
            Status::Expired => "expired",
            Status::Urgent => "urgent",
            Status::Soon => "soon",
            Status::Ok => "ok",
        }
    }
}

/// A batch with its expiry worked out for a given day.
#[derive(Debug, Clone, Serialize)]
pub struct Dated {
    #[serde(flatten)]
    pub batch: Batch,
    pub days_left: Option<i64>,
    pub status: Status,
}

pub fn annotate(batches: Vec<Batch>, today: NaiveDate) -> Vec<Dated> {
    batches
        .into_iter()
        .map(|b| {
            let d = days_left(b.expires_on.as_deref(), today);
            Dated {
                batch: b,
                days_left: d,
                status: Status::of(d, 14),
            }
        })
        .collect()
}

/// Everything a page needs about the stock, read once.
pub struct Snapshot {
    pub batches: Vec<Dated>,
    pub items: Vec<Item>,
    pub today: NaiveDate,
}

impl Snapshot {
    pub fn new(items: Vec<Item>, batches: Vec<Batch>, today: NaiveDate) -> Self {
        Self {
            batches: annotate(batches, today),
            items,
            today,
        }
    }

    /// Batches already expired or expiring within `within` days, soonest first.
    pub fn expiring(&self, within: i64) -> Vec<Dated> {
        let mut out: Vec<Dated> = self
            .batches
            .iter()
            .filter(|b| b.days_left.is_some_and(|d| d <= within))
            .cloned()
            .collect();
        out.sort_by_key(|b| b.days_left);
        out
    }

    pub fn low_stock(&self) -> Vec<Item> {
        self.items.iter().filter(|i| i.is_low()).cloned().collect()
    }
}

/// Batches already expired or expiring within `within` days, soonest first.
pub fn expiring(batches: Vec<Batch>, today: NaiveDate, within: i64) -> Vec<Dated> {
    let mut out: Vec<Dated> = annotate(batches, today)
        .into_iter()
        .filter(|b| b.days_left.is_some_and(|d| d <= within))
        .collect();
    out.sort_by_key(|b| b.days_left);
    out
}

pub fn low_stock(items: Vec<Item>) -> Vec<Item> {
    items.into_iter().filter(Item::is_low).collect()
}

/// How much `qty` units of this item are, in base units, for matching targets.
/// A "pcs" item with a pack size (e.g. 1.5 L bottles) counts by its contents.
pub fn item_amount(item: &Item, qty: f64) -> (Dim, f64) {
    if matches!(item.unit.as_str(), "g" | "kg" | "ml" | "L")
        || CURRENCIES.contains(&item.unit.as_str())
    {
        return to_base(qty, &item.unit);
    }
    if let (Some(size), Some(unit)) = (item.size, item.size_unit.as_deref()) {
        return to_base(qty * size, unit);
    }
    (Dim::Count, qty)
}

/// `1`, `0.5`, `2.25` — no trailing zeros, like Python's `{:g}`.
pub fn fmt_qty(v: f64) -> String {
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.into()
    }
}
