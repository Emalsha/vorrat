//! The daily reminder digest: what expires, what is low, which medicine runs out.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use chrono::NaiveDate;
use serde::Serialize;

use crate::db::Store;
use crate::essentials;
use crate::essentials::rules::MED_REMIND_DAYS;
use crate::stock::queries::Item;
use crate::stock::rules::{annotate, fmt_qty, low_stock, Dated, Snapshot};

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
    let meds = essentials::rules::medicines(store, &snap)?;
    let sent = store.dose_marks()?;
    let who = essentials::rules::member_labels(&store.members()?);
    let remind = MED_REMIND_DAYS;
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
