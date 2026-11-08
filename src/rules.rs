//! Rules on top of the store: expiry status, low stock, emergency coverage and
//! the daily reminder digest. Kept apart from the web layer so the same logic
//! serves the HTML pages, the JSON API and the scheduler.

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::Result;
use chrono::NaiveDate;
use serde::Serialize;

use crate::db::{Batch, Item, Store, Target};

/// Measurement dimension. Quantities only add up within one dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Dim {
    Mass,
    Volume,
    Count,
}

/// Convert to the base unit of its dimension: g, ml or pcs.
pub fn to_base(value: f64, unit: &str) -> (Dim, f64) {
    match unit {
        "g" => (Dim::Mass, value),
        "kg" => (Dim::Mass, value * 1000.0),
        "ml" => (Dim::Volume, value),
        "L" => (Dim::Volume, value * 1000.0),
        _ => (Dim::Count, value),
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
    pub fn load(store: &Store, today: NaiveDate) -> Result<Self> {
        Ok(Self {
            batches: annotate(store.batches(None)?, today),
            items: store.all_items()?,
            today,
        })
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
pub fn expiring(store: &Store, today: NaiveDate, within: i64) -> Result<Vec<Dated>> {
    let mut out: Vec<Dated> = annotate(store.batches(None)?, today)
        .into_iter()
        .filter(|b| b.days_left.is_some_and(|d| d <= within))
        .collect();
    out.sort_by_key(|b| b.days_left);
    Ok(out)
}

pub fn low_stock(store: &Store) -> Result<Vec<Item>> {
    Ok(store
        .all_items()?
        .into_iter()
        .filter(Item::is_low)
        .collect())
}

/// How much `qty` units of this item are, in base units, for matching targets.
/// A "pcs" item with a pack size (e.g. 1.5 L bottles) counts by its contents.
pub fn item_amount(item: &Item, qty: f64) -> (Dim, f64) {
    if matches!(item.unit.as_str(), "g" | "kg" | "ml" | "L") {
        return to_base(qty, &item.unit);
    }
    if let (Some(size), Some(unit)) = (item.size, item.size_unit.as_deref()) {
        return to_base(qty * size, unit);
    }
    (Dim::Count, qty)
}

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
#[derive(Debug, Clone, Serialize)]
pub struct Coverage {
    pub target: Target,
    pub need: f64,
    pub have: f64,
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

/// Emergency plan: per target, what we need vs. what we have. Expired batches
/// don't count. Items match a target by category.
pub fn coverage(store: &Store, today: NaiveDate) -> Result<Plan> {
    coverage_of(store, &Snapshot::load(store, today)?)
}

/// [`coverage`] from an already loaded snapshot.
/// Each target's mode says who it is for and whether it is needed every day.
pub fn coverage_of(store: &Store, snap: &Snapshot) -> Result<Plan> {
    let (people, pets) = store.household_counts()?;
    let days = store.setting_int("days", 14).max(1);
    let batches: Vec<&Dated> = snap
        .batches
        .iter()
        .filter(|b| b.status != Status::Expired)
        .collect();
    let items: HashMap<i64, &Item> = snap.items.iter().map(|i| (i.id, i)).collect();
    let mut lines = Vec::new();
    for t in store.targets()? {
        let (count, per_day) = match t.mode.as_str() {
            "per_person_day" => (people, true),
            "per_person" => (people, false),
            "per_pet_day" => (pets, true),
            "per_pet" => (pets, false),
            _ => (1, false),
        };
        let need = t.amount * (count * if per_day { days } else { 1 }) as f64;
        let (tdim, tfactor) = to_base(1.0, &t.unit);
        let mut have_base = 0.0;
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
            *contrib.entry(it.id).or_default() += amt / tfactor;
        }
        let have = round3(have_base / tfactor);
        let need = round3(need);
        let ratio = if need > 0.0 {
            (have / need).min(1.0)
        } else {
            1.0
        };
        let days_cov = (per_day && t.amount > 0.0 && count > 0)
            .then(|| round1(have / (t.amount * count as f64)));
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
        crate::essentials::medicine_days(&crate::essentials::medicines(store, snap)?);
    let weakest_days = lines
        .iter()
        .filter_map(|c| c.days)
        .chain(medicine_days)
        .reduce(f64::min);
    Ok(Plan {
        people,
        pets,
        days,
        lines,
        ratio,
        weakest_days,
        medicine_days,
    })
}

/// Reminder marks in days before expiry; 0 means "has expired".
#[derive(Debug, Clone)]
pub struct RemindDays {
    pub default: Vec<i64>,
    pub per_kind: HashMap<String, Vec<i64>>,
}

impl Default for RemindDays {
    fn default() -> Self {
        Self {
            default: vec![14, 7, 0],
            per_kind: HashMap::from([("document".into(), vec![90, 30, 0])]),
        }
    }
}

impl RemindDays {
    fn for_kind(&self, kind: &str) -> Vec<i64> {
        let mut m = self.per_kind.get(kind).unwrap_or(&self.default).clone();
        m.sort_unstable_by(|a, b| b.cmp(a));
        m
    }
}

/// What the daily reminder says.
///
/// - `week`: crossed the middle mark (7 days, 30 for documents).
/// - `soon`: crossed the first mark (14 days, 90 for documents).
/// - `medicine`: medicines that last fewer than `MED_REMIND_DAYS` days.
/// - `renew`: prescriptions to renew within `MED_REMIND_DAYS` days.
#[derive(Debug, Default, Serialize)]
pub struct Digest {
    pub expired: Vec<Dated>,
    pub week: Vec<Dated>,
    pub soon: Vec<Dated>,
    pub low: Vec<Item>,
    pub medicine: Vec<MedAlert>,
    pub renew: Vec<MedAlert>,
    #[serde(skip)]
    marks: Vec<(i64, Vec<i64>)>,
    #[serde(skip)]
    low_marks: Vec<i64>,
    #[serde(skip)]
    dose_marks: Vec<(i64, &'static str)>,
}

/// A medicine reminder: which one, for whom, and how many days are left.
///
/// - `days`: days of stock left, or days to the renewal date.
#[derive(Debug, Clone, Serialize)]
pub struct MedAlert {
    pub item_id: i64,
    pub name: String,
    pub who: Option<String>,
    pub days: f64,
}

impl Digest {
    pub fn is_empty(&self) -> bool {
        self.expired.is_empty()
            && self.week.is_empty()
            && self.soon.is_empty()
            && self.low.is_empty()
            && self.medicine.is_empty()
            && self.renew.is_empty()
    }

    pub fn title(&self) -> String {
        let mut parts = Vec::new();
        for (n, what) in [
            (self.expired.len(), "expired"),
            (self.week.len(), "expiring this week"),
            (self.soon.len(), "expiring soon"),
            (self.low.len(), "running low"),
            (self.medicine.len(), "medicine running out"),
            (self.renew.len(), "prescription to renew"),
        ] {
            if n > 0 {
                parts.push(format!("{n} {}", crate::i18n::tr(what)));
            }
        }
        format!("Vorrat: {}", parts.join(", "))
    }

    pub fn body(&self) -> String {
        let fmt = |b: &Dated| {
            let d = b.days_left.unwrap_or(0);
            use crate::i18n::trf;
            let when = match d {
                0 => crate::i18n::tr("today"),
                d if d < 0 => trf("{0}d ago", &[&-d]),
                d => trf("in {0}d", &[&d]),
            };
            format!(
                "{} ({} {}, {when})",
                b.batch.item_name,
                fmt_qty(b.batch.qty),
                crate::i18n::tr(&b.batch.unit)
            )
        };
        let mut lines = Vec::new();
        for (label, rows) in [
            ("Expired", &self.expired),
            ("This week", &self.week),
            ("Soon", &self.soon),
        ] {
            if !rows.is_empty() {
                lines.push(format!(
                    "{}: {}",
                    crate::i18n::tr(label),
                    rows.iter().map(fmt).collect::<Vec<_>>().join("; ")
                ));
            }
        }
        if !self.low.is_empty() {
            let low: Vec<String> = self
                .low
                .iter()
                .map(|i| {
                    format!(
                        "{} ({}/{} {})",
                        i.name,
                        fmt_qty(i.total),
                        fmt_qty(i.min_qty),
                        crate::i18n::tr(&i.unit)
                    )
                })
                .collect();
            lines.push(format!(
                "{}: {}",
                crate::i18n::tr("Running low"),
                low.join("; ")
            ));
        }
        for (label, rows, fmt) in [
            ("Medicine running out", &self.medicine, "{0} days left"),
            ("Prescription to renew", &self.renew, "in {0}d"),
        ] {
            if rows.is_empty() {
                continue;
            }
            let txt: Vec<String> = rows
                .iter()
                .map(|m| {
                    let days = if m.days < 0.0 {
                        crate::i18n::tr("overdue")
                    } else {
                        crate::i18n::trf(fmt, &[&fmt_qty(m.days)])
                    };
                    match &m.who {
                        Some(w) => format!("{} ({w}, {days})", m.name),
                        None => format!("{} ({days})", m.name),
                    }
                })
                .collect();
            lines.push(format!("{}: {}", crate::i18n::tr(label), txt.join("; ")));
        }
        lines.join("\n")
    }
}

/// What's NEW to tell the user today. Each batch is reminded once per mark; if
/// a batch crossed several marks since last time, only the most urgent is sent
/// and the earlier ones are recorded as done.
/// A low-stock alert is cleared when the item is restocked, so the next drop
/// alerts again. Medicines are reminded once when the stock gets short (again
/// after a restock) and once per renewal date.
pub fn build_digest(store: &Store, today: NaiveDate, remind: &RemindDays) -> Result<Digest> {
    let mut dg = Digest::default();
    for b in annotate(store.batches(None)?, today) {
        let Some(d) = b.days_left else { continue };
        let marks = remind.for_kind(&b.batch.kind);
        let crossed: Vec<i64> = marks
            .iter()
            .copied()
            .filter(|&m| if m == 0 { d < 0 } else { d <= m })
            .collect();
        if crossed.is_empty() {
            continue;
        }
        let done = store.notified_thresholds(b.batch.id)?;
        let fresh: Vec<i64> = crossed
            .iter()
            .copied()
            .filter(|m| !done.contains(m))
            .collect();
        if fresh.is_empty() {
            continue;
        }
        let most = *crossed.iter().min().expect("non-empty");
        let id = b.batch.id;
        if !done.contains(&most) {
            if most == 0 {
                dg.expired.push(b);
            } else if most == marks[0] {
                dg.soon.push(b);
            } else {
                dg.week.push(b);
            }
        }
        dg.marks.push((id, fresh));
    }
    let low_done = store.low_notified()?;
    let low_now: Vec<Item> = low_stock(store)?;
    let low_ids: HashSet<i64> = low_now.iter().map(|i| i.id).collect();
    for iid in low_done.difference(&low_ids) {
        store.clear_low(*iid)?;
    }
    for it in low_now {
        if !low_done.contains(&it.id) {
            dg.low_marks.push(it.id);
            dg.low.push(it);
        }
    }
    let snap = Snapshot::load(store, today)?;
    let meds = crate::essentials::medicines(store, &snap)?;
    let sent = store.dose_marks()?;
    let who = crate::essentials::member_labels(&store.members()?);
    let remind = crate::essentials::MED_REMIND_DAYS;
    for m in meds {
        let alert = |days: f64| MedAlert {
            item_id: m.item.id,
            name: m.item.name.clone(),
            who: m.dose.member_id.and_then(|id| who.get(&id).cloned()),
            days,
        };
        let low_sent = sent.contains(&(m.item.id, "low".to_string()));
        if m.days < remind as f64 {
            if !low_sent {
                dg.medicine.push(alert(m.days));
                dg.dose_marks.push((m.item.id, "low"));
            }
        } else if low_sent {
            store.clear_dose_mark(m.item.id, "low")?;
        }
        if let Some(d) = m.renew_in.filter(|d| *d <= remind) {
            if !sent.contains(&(m.item.id, "renew".to_string())) {
                dg.renew.push(alert(d as f64));
                dg.dose_marks.push((m.item.id, "renew"));
            }
        }
    }
    Ok(dg)
}

/// Record a digest as sent, so the same reminder never repeats.
pub fn commit_digest(store: &Store, dg: &Digest) -> Result<()> {
    for (bid, ts) in &dg.marks {
        store.mark_notified(*bid, ts)?;
    }
    for iid in &dg.low_marks {
        store.mark_low(*iid)?;
    }
    for (iid, mark) in &dg.dose_marks {
        store.mark_dose(*iid, mark)?;
    }
    Ok(())
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
/// each soonest-expiring first. Documents and non-food are left out.
pub struct PowerCut {
    pub fridge: Vec<Dated>,
    pub freezer: Vec<Dated>,
}

/// Sorts food into fridge and freezer. A location counts as cold through its
/// parents too, so a box inside the freezer is in the freezer.
pub fn power_cut(store: &Store, today: NaiveDate) -> Result<PowerCut> {
    let locs = store.locations()?;
    let by_id: std::collections::HashMap<i64, &crate::db::Location> =
        locs.iter().map(|l| (l.id, l)).collect();
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
    for d in annotate(store.batches(None)?, today) {
        if d.batch.kind != "food" {
            continue;
        }
        match kind_of(d.batch.location_id) {
            Some(Cold::Fridge) => fridge.push(d),
            Some(Cold::Freezer) => freezer.push(d),
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
    Ok(PowerCut { fridge, freezer })
}
