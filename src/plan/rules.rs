//! Emergency coverage: how many days the stock lasts for the household, and what to eat first in a power cut.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::NaiveDate;
use serde::Serialize;

use crate::essentials;
use crate::essentials::queries::Dose;
use crate::locations::queries::Location;
use crate::plan::queries::Target;
use crate::stock::queries::{Batch, Item};
use crate::stock::rules::{annotate, item_amount, to_base, Dated, Snapshot, Status};

/// What one item adds to a plan line.
///
/// - `amount`: in the target's unit.
#[derive(Debug, Clone, Serialize)]
pub struct Contribution {
    pub item_id: i64,
    pub name: String,
    pub amount: f64,
}

/// How far the stock covers one plan line.
///
/// - `need`: in the target's unit.
/// - `ratio`: 0..=1
/// - `days`: for per-day targets (people or pets): how many days the stock lasts.
/// - `no_cook`: the part of `have` that can be eaten without cooking.
/// - `no_cook_days`: for per-day targets, how many days that part lasts.
#[derive(Debug, Clone, Serialize)]
pub struct Coverage {
    pub target: Target,
    pub need: f64,
    pub have: f64,
    pub no_cook: f64,
    pub no_cook_days: Option<f64>,
    pub items: Vec<Contribution>,
    pub ratio: f64,
    pub missing: f64,
    pub days: Option<f64>,
}

/// The whole emergency plan against the stock.
///
/// - `ratio`: average coverage across targets, 0..=1.
/// - `weakest_days`: days the household gets through on its weakest essential (per-day
///   line or medicine).
/// - `medicine_days`: days until the first medicine runs out, when anyone takes one.
#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub people: i64,
    pub pets: i64,
    pub days: i64,
    pub lines: Vec<Coverage>,
    pub ratio: f64,
    pub weakest_days: Option<f64>,
    pub medicine_days: Option<f64>,
}

impl Plan {
    pub fn gaps(&self) -> impl Iterator<Item = &Coverage> {
        self.lines.iter().filter(|c| c.missing > 0.0)
    }
}

/// What the plan needs besides the stock: the household, the plan length in
/// days, the targets and the medicine doses.
///
/// - `adults`, `children`: the people split for targets per adult or per child
///   (babies, children and teens count as children).
pub struct PlanInput {
    pub people: i64,
    pub pets: i64,
    pub adults: i64,
    pub children: i64,
    pub days: i64,
    pub targets: Vec<Target>,
    pub doses: Vec<Dose>,
}

/// Emergency plan: per target, what we need vs. what we have. Expired batches
/// don't count. Items match a target by category. Each target's mode says who
/// it is for and whether it is needed every day.
pub fn coverage(snap: &Snapshot, input: PlanInput) -> Plan {
    let PlanInput {
        people,
        pets,
        adults,
        children,
        days,
        targets,
        doses,
    } = input;
    let batches: Vec<&Dated> = snap
        .batches
        .iter()
        .filter(|b| b.status != Status::Expired)
        .collect();
    let items: HashMap<i64, &Item> = snap.items.iter().map(|i| (i.id, i)).collect();
    let mut lines = Vec::new();
    for t in targets {
        let (count, per_day) = match t.mode.as_str() {
            "per_person_day" => (people, true),
            "per_person" => (people, false),
            "per_pet_day" => (pets, true),
            "per_pet" => (pets, false),
            "per_adult" => (adults, false),
            "per_child" => (children, false),
            _ => (1, false),
        };
        let need = t.amount * (count * if per_day { days } else { 1 }) as f64;
        let (tdim, tfactor) = to_base(1.0, &t.unit);
        let mut have_base = 0.0;
        let mut no_cook_base = 0.0;
        let mut contrib: BTreeMap<i64, f64> = BTreeMap::new();
        for b in &batches {
            let Some(it) = items.get(&b.batch.item_id) else {
                continue;
            };
            if it.category.as_deref().unwrap_or("") != t.category {
                continue;
            }
            let (dim, amt) = item_amount(it, b.batch.qty);
            if dim != tdim {
                continue;
            }
            have_base += amt;
            if it.no_cook {
                no_cook_base += amt;
            }
            *contrib.entry(it.id).or_default() += amt / tfactor;
        }
        let have = round3(have_base / tfactor);
        let need = round3(need);
        let ratio = if need > 0.0 {
            (have / need).min(1.0)
        } else {
            1.0
        };
        let no_cook = round3(no_cook_base / tfactor);
        let per_day_need =
            (per_day && t.amount > 0.0 && count > 0).then_some(t.amount * count as f64);
        let days_cov = per_day_need.map(|n| round1(have / n));
        let no_cook_days = per_day_need
            .filter(|_| no_cook > 0.0)
            .map(|n| round1(no_cook / n));
        lines.push(Coverage {
            items: contrib
                .into_iter()
                .map(|(id, a)| Contribution {
                    item_id: id,
                    name: items[&id].name.clone(),
                    amount: round3(a),
                })
                .collect(),
            missing: round3((need - have).max(0.0)),
            ratio,
            need,
            have,
            no_cook,
            no_cook_days,
            days: days_cov,
            target: t,
        });
    }
    let ratio = if lines.is_empty() {
        1.0
    } else {
        lines.iter().map(|c| c.ratio).sum::<f64>() / lines.len() as f64
    };
    let medicine_days =
        essentials::rules::medicine_days(&essentials::rules::medicines(doses, snap));
    let weakest_days = lines
        .iter()
        .filter_map(|c| c.days)
        .chain(medicine_days)
        .reduce(f64::min);
    Plan {
        people,
        pets,
        days,
        lines,
        ratio,
        weakest_days,
        medicine_days,
    }
}

/// Days between two checks of the whole kit: about six months. The Dutch
/// guide says to check the kit every six months, the French once or twice a
/// year.
pub const REVIEW_DAYS: i64 = 182;

/// Days until the next kit check is due (negative when overdue), counted from
/// the last check, or from when Vorrat started counting.
pub fn review_in(since: NaiveDate, today: NaiveDate) -> i64 {
    REVIEW_DAYS - (today - since).num_days()
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Where a location keeps things cold, judged by its own or a parent's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cold {
    Fridge,
    Freezer,
}

pub fn cold_kind(name: &str) -> Option<Cold> {
    let n = name.trim().to_lowercase();
    if ["freez", "gefrier", "tiefkühl", "congél", "vries", "frys"]
        .iter()
        .any(|k| n.contains(k))
    {
        Some(Cold::Freezer)
    } else if [
        "fridge",
        "refrigerator",
        "kühlschrank",
        "frigo",
        "koelkast",
        "kylskåp",
    ]
    .iter()
    .any(|k| n.contains(k))
    {
        Some(Cold::Fridge)
    } else {
        None
    }
}

/// What to eat first when the power is off: the fridge, then the freezer,
/// each soonest-expiring first, then food elsewhere that needs no cooking.
/// Documents and non-food are left out.
pub struct PowerCut {
    pub fridge: Vec<Dated>,
    pub freezer: Vec<Dated>,
    pub no_cook: Vec<Dated>,
}

/// Sorts food into fridge, freezer and the rest. A location counts as cold
/// through its parents too, so a box inside the freezer is in the freezer.
/// `no_cook` holds the items that can be eaten without cooking; outside the
/// fridge and freezer, only those are listed. Expired batches are left out
/// of that list.
pub fn power_cut(
    locs: &[Location],
    batches: Vec<Batch>,
    no_cook: &HashSet<i64>,
    today: NaiveDate,
) -> PowerCut {
    let by_id: HashMap<i64, &Location> = locs.iter().map(|l| (l.id, l)).collect();
    let kind_of = |mut id: Option<i64>| {
        for _ in 0..16 {
            let l = by_id.get(&id?)?;
            if let Some(k) = cold_kind(&l.name) {
                return Some(k);
            }
            id = l.parent_id;
        }
        None
    };
    let mut fridge = Vec::new();
    let mut freezer = Vec::new();
    let mut rest = Vec::new();
    for d in annotate(batches, today) {
        if d.batch.kind != "food" {
            continue;
        }
        match kind_of(d.batch.location_id) {
            Some(Cold::Fridge) => fridge.push(d),
            Some(Cold::Freezer) => freezer.push(d),
            None if no_cook.contains(&d.batch.item_id) && d.status != Status::Expired => {
                rest.push(d)
            }
            None => {}
        }
    }
    let soonest = |a: &Dated, b: &Dated| {
        a.days_left
            .unwrap_or(i64::MAX)
            .cmp(&b.days_left.unwrap_or(i64::MAX))
    };
    fridge.sort_by(soonest);
    freezer.sort_by(soonest);
    rest.sort_by(soonest);
    PowerCut {
        fridge,
        freezer,
        no_cook: rest,
    }
}
