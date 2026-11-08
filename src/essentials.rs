//! Medicine and the two checklists: the first-aid kit and the things every
//! household should have once (radio, torch, cash...).
//!
//! The checklist entries come from the official guides gathered in
//! `docs/guidance/presets.json`: each one names the countries whose verified
//! guide lists it. No amounts are invented (AGENTS.md rule 7): an entry is
//! ticked when the household has at least one, or one per person where the
//! guides say so. No entry is a medicine: Vorrat never suggests one.

use std::collections::HashMap;

use anyhow::Result;
use chrono::NaiveDate;
use serde::Serialize;

use crate::db::{Dose, Item, Member, Store};
use crate::rules::{Dated, Snapshot, Status};

/// One entry of a checklist.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Check {
    /// Stored in `items.preset`; never change one.
    pub key: &'static str,
    /// English; translated on the page. Also the name of an item made from it.
    pub label: &'static str,
    /// The category an item made from it gets.
    pub category: &'static str,
    /// One for each person instead of one for the home.
    pub per_person: bool,
    /// Country codes of the verified official guides that name it.
    pub named_by: &'static [&'static str],
}

const fn c(
    key: &'static str,
    label: &'static str,
    category: &'static str,
    named_by: &'static [&'static str],
) -> Check {
    Check {
        key,
        label,
        category,
        per_person: false,
        named_by,
    }
}

pub const FIRST_AID: &[Check] = &[
    c(
        "fa-plasters",
        "Plasters",
        "first aid",
        &["HR", "CZ", "DK", "SI"],
    ),
    c(
        "fa-bandages",
        "Bandages and dressings",
        "first aid",
        &["HR", "CZ", "DK", "FR", "LV", "PL", "SI", "CH"],
    ),
    c(
        "fa-gauze",
        "Gauze or compresses",
        "first aid",
        &["FR", "PL", "SI"],
    ),
    c(
        "fa-disinfectant",
        "Wound disinfectant",
        "first aid",
        &["HR", "CZ", "DK", "FR", "LV", "PL", "SI"],
    ),
    c("fa-gloves", "Disposable gloves", "first aid", &["CZ", "PL"]),
    c(
        "fa-thermometer",
        "Thermometer",
        "first aid",
        &["CZ", "LV", "PL", "SI", "CH"],
    ),
    c(
        "fa-scissors",
        "Scissors",
        "first aid",
        &["CZ", "LV", "PL", "SI"],
    ),
    c("fa-tweezers", "Tweezers", "first aid", &["CZ", "SI"]),
    c(
        "fa-blanket",
        "Rescue blanket",
        "first aid",
        &["CZ", "LV", "PL"],
    ),
    c(
        "fa-masks",
        "Face masks",
        "first aid",
        &["HR", "CZ", "FR", "PL", "CH"],
    ),
    c("fa-guide", "First-aid instructions", "first aid", &["NL"]),
];

pub const ESSENTIALS: &[Check] = &[
    c(
        "es-radio",
        "Battery or wind-up radio",
        "radio",
        &[
            "BE", "HR", "CZ", "DK", "EE", "FI", "FR", "DE", "IE", "LV", "NL", "PL", "SI", "SE",
            "CH",
        ],
    ),
    c(
        "es-torch",
        "Torch",
        "light",
        &[
            "BE", "HR", "CZ", "DK", "EE", "FI", "FR", "DE", "IE", "LV", "NL", "PL", "SI", "CH",
        ],
    ),
    c(
        "es-batteries",
        "Spare batteries",
        "batteries",
        &[
            "BE", "HR", "CZ", "DK", "EE", "FR", "DE", "IE", "LV", "NL", "PL",
        ],
    ),
    c(
        "es-powerbank",
        "Power bank",
        "batteries",
        &[
            "BE", "CZ", "DK", "EE", "FI", "FR", "IE", "LV", "NL", "PL", "SE",
        ],
    ),
    c(
        "es-candles",
        "Candles and matches",
        "light",
        &[
            "BE", "HR", "DK", "EE", "FR", "DE", "IE", "LV", "NL", "PL", "SI", "SE", "CH",
        ],
    ),
    c(
        "es-stove",
        "Camping stove with fuel",
        "cooking",
        &["CZ", "DK", "EE", "FR", "DE", "LV", "SI", "SE", "CH"],
    ),
    c("es-opener", "Can opener", "cooking", &["BE", "SI"]),
    c(
        "es-cash",
        "Cash in small notes",
        "cash",
        &[
            "BE", "HR", "CZ", "DK", "EE", "FI", "FR", "DE", "IE", "LV", "NL", "PL", "SI", "SE",
            "CH",
        ],
    ),
    c(
        "es-documents",
        "Copies of important documents",
        "documents",
        &["BE", "HR", "CZ", "FR", "DE", "IE", "LV", "NL", "PL", "SI"],
    ),
    c(
        "es-phones",
        "Phone numbers on paper",
        "documents",
        &["BE", "HR", "DK", "LV", "NL"],
    ),
    Check {
        per_person: true,
        ..c(
            "es-blankets",
            "Blanket or sleeping bag",
            "warmth",
            &["BE", "HR", "DK", "FR", "IE", "LV", "NL", "SI", "SE"],
        )
    },
    c(
        "es-toilet",
        "Toilet paper",
        "hygiene",
        &[
            "BE", "HR", "CZ", "DK", "DE", "LV", "NL", "PL", "SI", "SE", "CH",
        ],
    ),
    c(
        "es-soap",
        "Soap or hand gel",
        "hygiene",
        &["BE", "HR", "CZ", "DK", "FR", "NL", "SI", "SE", "CH"],
    ),
    c(
        "es-period",
        "Menstrual products",
        "hygiene",
        &["BE", "HR", "DK", "NL", "PL", "SI", "SE"],
    ),
    c(
        "es-bags",
        "Rubbish bags",
        "hygiene",
        &["CZ", "PL", "SI", "SE"],
    ),
];

/// The checklist entry with this key.
pub fn find(key: &str) -> Option<&'static Check> {
    FIRST_AID.iter().chain(ESSENTIALS).find(|c| c.key == key)
}

/// Where a checklist entry stands in this household.
#[derive(Debug, Clone, Serialize)]
pub struct CheckState {
    pub check: Check,
    /// The items standing for it.
    pub items: Vec<(i64, String)>,
    /// Usable (not expired) amount, counted in the items' own units.
    pub have: f64,
    pub need: f64,
    /// Some stock has expired.
    pub expired: bool,
}

impl CheckState {
    pub fn ok(&self) -> bool {
        self.have >= self.need
    }
}

pub fn checklist(list: &[Check], snap: &Snapshot, people: i64) -> Vec<CheckState> {
    let mut by_key: HashMap<&str, Vec<&Item>> = HashMap::new();
    for it in &snap.items {
        if let Some(k) = it.preset.as_deref() {
            by_key.entry(k).or_default().push(it);
        }
    }
    list.iter()
        .map(|c| {
            let items = by_key.get(c.key).cloned().unwrap_or_default();
            let mut have = 0.0;
            let mut expired = false;
            for b in snap
                .batches
                .iter()
                .filter(|b| items.iter().any(|i| i.id == b.batch.item_id))
            {
                if b.status == Status::Expired {
                    expired = true;
                } else {
                    have += b.batch.qty;
                }
            }
            CheckState {
                check: *c,
                items: items.iter().map(|i| (i.id, i.name.clone())).collect(),
                have,
                need: if c.per_person {
                    people.max(1) as f64
                } else {
                    1.0
                },
                expired,
            }
        })
        .collect()
}

/// A medicine someone takes, with how long the stock lasts.
#[derive(Debug, Clone, Serialize)]
pub struct Medicine {
    pub item: Item,
    pub dose: Dose,
    /// Usable stock in the item's unit.
    pub have: f64,
    /// Days the usable stock lasts at the daily dose.
    pub days: f64,
    /// The soonest date some stock expires, among usable batches.
    pub next_expiry: Option<String>,
    /// Days to the prescription renewal; negative when overdue.
    pub renew_in: Option<i64>,
}

pub fn medicines(store: &Store, snap: &Snapshot) -> Result<Vec<Medicine>> {
    let items: HashMap<i64, &Item> = snap.items.iter().map(|i| (i.id, i)).collect();
    let mut out = Vec::new();
    for dose in store.doses()? {
        let Some(item) = items.get(&dose.item_id) else {
            continue;
        };
        let usable: Vec<&Dated> = snap
            .batches
            .iter()
            .filter(|b| b.batch.item_id == item.id && b.status != Status::Expired)
            .collect();
        let have: f64 = usable.iter().map(|b| b.batch.qty).sum();
        out.push(Medicine {
            days: (have / dose.per_day * 10.0).floor() / 10.0,
            next_expiry: usable
                .iter()
                .filter_map(|b| b.batch.expires_on.clone())
                .min(),
            renew_in: dose
                .renew_on
                .as_deref()
                .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
                .map(|d| (d - snap.today).num_days()),
            item: (*item).clone(),
            have,
            dose,
        });
    }
    Ok(out)
}

/// Days the household's medicine lasts: the medicine that runs out first.
pub fn medicine_days(meds: &[Medicine]) -> Option<f64> {
    meds.iter().map(|m| m.days).reduce(f64::min)
}

/// Remind when a medicine lasts fewer days than this, or its prescription
/// renewal is this close: time to see the doctor and the pharmacy.
pub const MED_REMIND_DAYS: i64 = 14;

/// How a person or pet is called on the page: the name, or the translated
/// type with a number ("Adult 2").
pub fn member_labels(members: &[Member]) -> HashMap<i64, String> {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    let mut out = HashMap::new();
    for m in members {
        let n = seen.entry(&m.r#type).or_default();
        *n += 1;
        let label = m.name.clone().unwrap_or_else(|| {
            let t = crate::i18n::tr(&m.r#type);
            let mut ch = t.chars();
            let t: String = ch
                .next()
                .map(|f| f.to_uppercase().chain(ch).collect())
                .unwrap_or_default();
            format!("{t} {n}")
        });
        out.insert(m.id, label);
    }
    out
}
