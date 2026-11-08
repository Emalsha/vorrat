//! Suggestions learned from your own history. The rule for everything here:
//! say nothing unless the evidence is strong. A wrong hint costs more than no
//! hint, so each function returns `None` when the data is thin or inconsistent,
//! and every suggestion carries a short human reason.
//!
//! Nothing here changes data; the web layer decides how to show it. The
//! thresholds sit together at the top so they are easy to tune.

use std::collections::HashMap;

use anyhow::Result;
use chrono::{Days, NaiveDate};
use serde::Serialize;

use crate::db::{Item, Location, Store};

pub const SHELF_MIN_SAMPLES: usize = 2;
pub const SHELF_MAX_SPREAD: f64 = 0.2;
pub const LOCATION_MIN_ITEMS: usize = 3;
pub const LOCATION_MIN_SHARE: f64 = 0.8;
pub const USE_MIN_EVENTS: usize = 3;
pub const USE_MIN_SPAN: i64 = 14;
pub const BUY_MIN_PURCHASES: usize = 4;
pub const BUY_MAX_CV: f64 = 0.35;
pub const BUY_LOOKBACK: u64 = 180;
pub const DISMISS_DAYS: u64 = 14;

fn day(ts: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(ts.get(..10)?, "%Y-%m-%d").ok()
}

pub fn human_days(days: f64) -> String {
    use crate::i18n::trf;
    let d = days.round();
    if d >= 330.0 {
        trf(
            "{0} yr",
            &[&crate::rules::fmt_qty((d / 365.0 * 10.0).round() / 10.0)],
        )
    } else if d >= 50.0 {
        trf("{0} mo", &[&(d / 30.4).round()])
    } else if d >= 14.0 {
        trf("{0} wk", &[&(d / 7.0).round()])
    } else {
        trf("{0} d", &[&d])
    }
}

fn median(v: &mut [i64]) -> f64 {
    v.sort_unstable();
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2] as f64
    } else {
        (v[n / 2 - 1] + v[n / 2]) as f64 / 2.0
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ShelfLife {
    pub days: i64,
    pub n: usize,
    pub label: String,
}

/// Typical days from purchase to printed expiry, if past purchases agree.
pub fn shelf_life(store: &Store, item_id: i64) -> Result<Option<ShelfLife>> {
    let mut spans: Vec<i64> = store
        .item_events(item_id, "add", "", 8)?
        .into_iter()
        .filter_map(|(ts, _, exp)| Some((day(exp.as_deref()?)? - day(&ts)?).num_days()))
        .filter(|s| *s > 0)
        .collect();
    if spans.len() < SHELF_MIN_SAMPLES {
        return Ok(None);
    }
    let n = spans.len();
    let mid = median(&mut spans);
    let tol = (mid * SHELF_MAX_SPREAD).max(4.0);
    let (lo, hi) = (spans[0] as f64, spans[n - 1] as f64);
    if hi - mid > tol || mid - lo > tol {
        return Ok(None);
    }
    let days = mid.round() as i64;
    Ok(Some(ShelfLife {
        days,
        n,
        label: human_days(days as f64),
    }))
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LocationGuess {
    pub id: i64,
    pub path: String,
    pub reason: String,
}

const FREEZER_NAMES: &[&str] = &[
    "freezer",
    "tiefkühler",
    "tiefkühltruhe",
    "gefrierschrank",
    "congélateur",
    "vriezer",
    "frys",
];

/// Where a NEW item probably goes.
pub fn guess_location(
    store: &Store,
    category: Option<&str>,
    frozen: bool,
) -> Result<Option<LocationGuess>> {
    let locs: Vec<Location> = store.locations()?;
    if frozen {
        if let Some(l) = locs
            .iter()
            .find(|l| FREEZER_NAMES.contains(&l.name.trim().to_lowercase().as_str()))
        {
            return Ok(Some(LocationGuess {
                id: l.id,
                path: l.path.clone(),
                reason: "frozen product".into(),
            }));
        }
    }
    let Some(cat) = category.filter(|c| !c.is_empty()) else {
        return Ok(None);
    };
    let votes: Vec<i64> = store
        .category_locations(cat)?
        .into_iter()
        .flatten()
        .collect();
    if votes.len() < LOCATION_MIN_ITEMS {
        return Ok(None);
    }
    let mut count: HashMap<i64, usize> = HashMap::new();
    for v in &votes {
        *count.entry(*v).or_default() += 1;
    }
    let (top, n) = count
        .into_iter()
        .max_by_key(|(id, n)| (*n, -*id))
        .expect("votes not empty");
    if (n as f64) / (votes.len() as f64) < LOCATION_MIN_SHARE {
        return Ok(None);
    }
    Ok(locs
        .into_iter()
        .find(|l| l.id == top)
        .map(|l| LocationGuess {
            id: l.id,
            path: l.path,
            reason: crate::i18n::trf(
                "where your other {0} are ({1} of {2})",
                &[&crate::i18n::tr(cat), &n, &votes.len()],
            ),
        }))
}

/// Quantity to prefill when restocking: only if the last 2 purchases agree.
pub fn usual_qty(store: &Store, item_id: i64) -> Result<Option<f64>> {
    let ev = store.item_events(item_id, "add", "", 2)?;
    Ok(match ev.as_slice() {
        [(_, Some(a), _), (_, Some(b), _)] if a == b && *a > 0.0 => Some(*a),
        _ => None,
    })
}

/// Units used per day over the last 90 days (thrown away doesn't count).
pub fn use_rate(store: &Store, item_id: i64, today: NaiveDate) -> Result<Option<f64>> {
    let since = (today - Days::new(90)).to_string();
    let ev = store.item_events(item_id, "use", &since, 1000)?;
    if ev.len() < USE_MIN_EVENTS {
        return Ok(None);
    }
    let Some(first) = ev.iter().filter_map(|(ts, _, _)| day(ts)).min() else {
        return Ok(None);
    };
    let span = (today - first).num_days();
    if span < USE_MIN_SPAN {
        return Ok(None);
    }
    Ok(Some(
        ev.iter().map(|(_, q, _)| q.unwrap_or(0.0)).sum::<f64>() / span as f64,
    ))
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BuyPattern {
    pub every: f64,
    pub n: usize,
    pub last: NaiveDate,
}

/// If you buy this at a regular interval.
pub fn buy_pattern(store: &Store, item_id: i64, today: NaiveDate) -> Result<Option<BuyPattern>> {
    let since = (today - Days::new(BUY_LOOKBACK)).to_string();
    let mut days: Vec<NaiveDate> = store
        .item_events(item_id, "add", &since, 1000)?
        .iter()
        .filter_map(|(ts, _, _)| day(ts))
        .collect();
    days.sort_unstable();
    days.dedup();
    if days.len() < BUY_MIN_PURCHASES {
        return Ok(None);
    }
    let gaps: Vec<f64> = days
        .windows(2)
        .map(|w| (w[1] - w[0]).num_days() as f64)
        .collect();
    let mean = gaps.iter().sum::<f64>() / gaps.len() as f64;
    if mean < 1.0 {
        return Ok(None);
    }
    let sd = (gaps.iter().map(|g| (g - mean).powi(2)).sum::<f64>() / gaps.len() as f64).sqrt();
    if sd / mean > BUY_MAX_CV {
        return Ok(None);
    }
    Ok(Some(BuyPattern {
        every: (mean * 10.0).round() / 10.0,
        n: days.len(),
        last: *days.last().expect("non-empty"),
    }))
}

/// Short facts for the item page, each only when it's well supported.
pub fn insights(store: &Store, item: &Item, today: NaiveDate) -> Result<Vec<String>> {
    let mut out = Vec::new();
    if item.kind != "document" {
        if let Some(sl) = shelf_life(store, item.id)? {
            out.push(crate::i18n::trf(
                "Usually keeps about {0} from purchase (last {1} purchases).",
                &[&sl.label, &sl.n],
            ));
        }
    }
    if let Some(rate) = use_rate(store, item.id, today)? {
        if item.total > 0.0 && rate > 0.0 {
            let left = item.total / rate;
            if left < 365.0 {
                let until = today + Days::new(left as u64);
                out.push(crate::i18n::trf(
                    "At your pace, what's here lasts until about {0}.",
                    &[&until.format(&crate::i18n::tr("%-d %b"))],
                ));
            }
        }
    }
    if let Some(bp) = buy_pattern(store, item.id, today)? {
        out.push(crate::i18n::trf(
            "You buy this about every {0} ({1} purchases).",
            &[&human_days(bp.every), &bp.n],
        ));
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize)]
pub struct Suggestion {
    pub item: Item,
    pub reason: String,
}

/// Items you'll very likely need soon that aren't on the list or flagged low
/// already: a steady rhythm and none left, or at your pace it runs out within 3 days.
pub fn shopping_suggestions(store: &Store, today: NaiveDate) -> Result<Vec<Suggestion>> {
    let listed: Vec<i64> = store
        .shopping()?
        .into_iter()
        .filter(|s| !s.done)
        .filter_map(|s| s.item_id)
        .collect();
    let hidden = store.dismissed(&today.to_string())?;
    let mut out = Vec::new();
    for it in store.all_items()? {
        if listed.contains(&it.id)
            || hidden.contains(&format!("shop:{}", it.id))
            || it.kind == "document"
            || it.is_low()
        {
            continue;
        }
        let mut reason = None;
        if it.total <= 0.0 {
            if let Some(bp) = buy_pattern(store, it.id, today)? {
                reason = Some(crate::i18n::trf(
                    "None left. You buy it about every {0}.",
                    &[&human_days(bp.every)],
                ));
            }
        } else if let Some(rate) = use_rate(store, it.id, today)? {
            let left = it.total / rate;
            if left <= 3.0 {
                let n = left.round().max(1.0);
                reason = Some(crate::i18n::trf(
                    "At your pace this runs out in about {0} d.",
                    &[&n],
                ));
            }
        }
        if let Some(r) = reason {
            out.push(Suggestion {
                item: it,
                reason: r,
            });
        }
    }
    Ok(out)
}

pub fn dismiss_suggestion(store: &Store, item_id: i64, today: NaiveDate) -> Result<()> {
    store.dismiss(
        &format!("shop:{item_id}"),
        &(today + Days::new(DISMISS_DAYS)).to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_days_labels() {
        assert_eq!(human_days(5.0), "5 d");
        assert_eq!(human_days(21.0), "3 wk");
        assert_eq!(human_days(152.0), "5 mo");
        assert_eq!(human_days(365.0), "1 yr");
    }
}
