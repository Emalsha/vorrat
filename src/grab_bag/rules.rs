//! Grab bags: one bag per person for leaving home quickly, apart from the home
//! stock. Belgium, Czechia, Ireland, Latvia, the Netherlands and Poland
//! describe one in their guides (`docs/guidance/presets.json`, category
//! `grab_bag`). A bag is a place in the house; an entry of the checklist is
//! ticked when that place, or a place inside it, holds stock of one of the
//! entry's categories that has not expired. No amounts: the guides give none.

use std::collections::HashSet;

use crate::locations::queries::Location;
use crate::stock::rules::{Dated, Status};

/// One entry of the grab-bag checklist.
///
/// - `label`: English; translated on the page.
/// - `categories`: stock in any of these categories ticks it.
/// - `named_by`: country codes of the verified guides that put it in the bag.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    pub label: &'static str,
    pub categories: &'static [&'static str],
    pub named_by: &'static [&'static str],
}

const fn e(
    label: &'static str,
    categories: &'static [&'static str],
    named_by: &'static [&'static str],
) -> Entry {
    Entry {
        label,
        categories,
        named_by,
    }
}

/// What the verified guides put in a grab bag, in the order a person packs it.
pub const ENTRIES: &[Entry] = &[
    e(
        "Copies of documents",
        &["documents"],
        &["BE", "CZ", "IE", "LV", "PL"],
    ),
    e("Cash", &["cash"], &["BE", "CZ", "IE", "LV", "PL"]),
    e(
        "Personal medicines for a few days",
        &["medicine"],
        &["BE", "CZ", "IE", "LV"],
    ),
    e("Water", &["water"], &["BE", "LV", "PL"]),
    e(
        "Food that keeps",
        &["snacks", "ready meals", "grains", "fruit", "protein"],
        &["BE", "LV", "PL"],
    ),
    e("First-aid kit", &["first aid"], &["BE", "LV", "PL"]),
    e("Torch", &["light"], &["CZ", "PL"]),
    e("Radio", &["radio"], &["CZ", "LV"]),
    e(
        "Power bank or phone charger",
        &["batteries"],
        &["BE", "CZ", "IE", "LV", "PL"],
    ),
    e("Hygiene items", &["hygiene"], &["IE", "PL"]),
    e(
        "Warm clothes or a sleeping bag",
        &["warmth"],
        &["CZ", "IE", "LV", "PL"],
    ),
    e(
        "Whistle or multitool",
        &["tools"],
        &["BE", "CZ", "HR", "LV", "NL", "PL"],
    ),
];

/// A place and every place inside it.
pub fn inside(root: i64, locations: &[Location]) -> HashSet<i64> {
    let mut out = HashSet::from([root]);
    loop {
        let before = out.len();
        for l in locations {
            if l.parent_id.is_some_and(|p| out.contains(&p)) {
                out.insert(l.id);
            }
        }
        if out.len() == before {
            return out;
        }
    }
}

/// One checklist entry for one bag.
///
/// - `batches`: the stock in the bag that counts for it, expired or not.
pub struct Line<'a> {
    pub entry: &'static Entry,
    pub batches: Vec<&'a Dated>,
}

impl Line<'_> {
    /// Something in the bag for this entry has not expired.
    pub fn ok(&self) -> bool {
        self.batches.iter().any(|b| b.status != Status::Expired)
    }

    pub fn expired(&self) -> bool {
        self.batches.iter().any(|b| b.status == Status::Expired)
    }
}

/// The checklist for the bag made of these places.
pub fn check<'a>(places: &HashSet<i64>, batches: &'a [Dated]) -> Vec<Line<'a>> {
    ENTRIES
        .iter()
        .map(|entry| Line {
            entry,
            batches: batches
                .iter()
                .filter(|b| b.batch.location_id.is_some_and(|l| places.contains(&l)))
                .filter(|b| {
                    b.batch
                        .category
                        .as_deref()
                        .is_some_and(|c| entry.categories.contains(&c))
                })
                .collect(),
        })
        .collect()
}
