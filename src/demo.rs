//! Sample household for a public demo (`VORRAT_DEMO=on`).

use anyhow::Result;
use chrono::{Days, NaiveDate};

use crate::db::{NewItem, Store};

/// (name, kind, category, unit, pack size, pack unit, min, [(qty, days until expiry, location)])
type Sample = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    Option<f64>,
    Option<&'static str>,
    f64,
    &'static [(f64, Option<i64>, &'static str)],
);

const SAMPLES: &[Sample] = &[
    (
        "Still water 1.5 L",
        "food",
        "water",
        "pcs",
        Some(1.5),
        Some("L"),
        12.0,
        &[(18.0, Some(300), "Cellar")],
    ),
    (
        "Rice",
        "food",
        "grains",
        "kg",
        None,
        None,
        2.0,
        &[(3.0, Some(400), "Pantry")],
    ),
    (
        "Pasta",
        "food",
        "grains",
        "pcs",
        Some(0.5),
        Some("kg"),
        4.0,
        &[(6.0, Some(250), "Pantry")],
    ),
    (
        "Canned lentils",
        "food",
        "vegetables",
        "pcs",
        Some(0.4),
        Some("kg"),
        4.0,
        &[(8.0, Some(700), "Pantry")],
    ),
    (
        "Canned tomatoes",
        "food",
        "vegetables",
        "pcs",
        Some(0.4),
        Some("kg"),
        0.0,
        &[(5.0, Some(500), "Pantry")],
    ),
    (
        "Long-life milk",
        "food",
        "dairy",
        "pcs",
        Some(1.0),
        Some("L"),
        2.0,
        &[(1.0, Some(5), "Fridge"), (4.0, Some(90), "Pantry")],
    ),
    (
        "Yoghurt",
        "food",
        "dairy",
        "pcs",
        Some(0.5),
        Some("kg"),
        0.0,
        &[(2.0, Some(1), "Fridge")],
    ),
    (
        "Frozen peas",
        "food",
        "vegetables",
        "pcs",
        Some(0.75),
        Some("kg"),
        0.0,
        &[(2.0, Some(180), "Freezer")],
    ),
    (
        "Peanut butter",
        "food",
        "fats",
        "pcs",
        Some(0.35),
        Some("kg"),
        0.0,
        &[(1.0, Some(-3), "Pantry")],
    ),
    (
        "Olive oil",
        "food",
        "fats",
        "L",
        None,
        None,
        1.0,
        &[(0.5, Some(400), "Pantry")],
    ),
    (
        "Muesli",
        "food",
        "grains",
        "pcs",
        Some(0.75),
        Some("kg"),
        2.0,
        &[(1.0, Some(12), "Pantry")],
    ),
    (
        "AA batteries",
        "supply",
        "batteries",
        "pcs",
        None,
        None,
        8.0,
        &[(4.0, None, "Cellar")],
    ),
    (
        "Torch",
        "thing",
        "light",
        "pcs",
        None,
        None,
        0.0,
        &[(1.0, None, "Cellar")],
    ),
    (
        "First-aid kit",
        "supply",
        "first aid",
        "pcs",
        None,
        None,
        1.0,
        &[(1.0, Some(40), "Bathroom")],
    ),
    (
        "Passport",
        "document",
        "documents",
        "pcs",
        None,
        None,
        0.0,
        &[(1.0, Some(80), "Office")],
    ),
];

pub fn seed(store: &Store, today: NaiveDate) -> Result<()> {
    let locs = store.locations()?;
    let loc = |name: &str| -> Result<i64> {
        match locs.iter().find(|l| l.name.eq_ignore_ascii_case(name)) {
            Some(l) => Ok(l.id),
            None => store.add_location(name, None),
        }
    };
    for (name, kind, cat, unit, size, size_unit, min, batches) in SAMPLES {
        let id = store.add_item(NewItem {
            name: name.to_string(),
            kind: Some(kind.to_string()),
            category: Some(cat.to_string()),
            unit: Some(unit.to_string()),
            size: *size,
            size_unit: size_unit.map(str::to_string),
            min_qty: Some(*min),
            ..Default::default()
        })?;
        for (qty, days, place) in *batches {
            let exp = days.map(|d| {
                if d >= 0 {
                    today + Days::new(d as u64)
                } else {
                    today - Days::new((-d) as u64)
                }
            });
            store.add_batch(id, *qty, exp, Some(loc(place)?), None)?;
        }
    }
    Ok(())
}

/// Back to the sample household.
pub fn reset(store: &Store, today: NaiveDate) -> Result<()> {
    store.wipe_stock()?;
    seed(store, today)
}
