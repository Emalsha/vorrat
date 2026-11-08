//! Queries for plan targets.

use anyhow::{bail, Result};
use rusqlite::params;
use serde::Serialize;

use crate::db::Store;
use crate::stock::queries::UNITS;

/// How a plan target scales with the household.
pub const TARGET_MODES: &[&str] = &[
    "per_person_day",
    "per_person",
    "per_pet_day",
    "per_pet",
    "fixed",
];

/// A line of the emergency plan.
///
/// - `mode`: one of [`TARGET_MODES`].
#[derive(Debug, Clone, Serialize)]
pub struct Target {
    pub id: i64,
    pub label: String,
    pub category: String,
    pub mode: String,
    pub amount: f64,
    pub unit: String,
    pub sort: i64,
}

fn validate_target(label: &str, mode: &str, amount: f64, unit: &str) -> Result<()> {
    if label.trim().is_empty() {
        bail!("label is required");
    }
    if !TARGET_MODES.contains(&mode) {
        bail!("unknown target mode {mode:?}");
    }
    if !(amount.is_finite() && amount >= 0.0) {
        bail!("amount must be zero or more");
    }
    if !UNITS.contains(&unit) {
        bail!("unknown unit {unit:?}");
    }
    Ok(())
}

impl Store {
    pub fn targets(&self) -> Result<Vec<Target>> {
        let c = self.r();
        let mut st = c.prepare_cached(
            "SELECT id, label, category, mode, amount, unit, sort FROM targets ORDER BY sort, id",
        )?;
        let v = st
            .query_map([], |r| {
                Ok(Target {
                    id: r.get("id")?,
                    label: r.get("label")?,
                    category: r.get("category")?,
                    mode: r.get("mode")?,
                    amount: r.get("amount")?,
                    unit: r.get("unit")?,
                    sort: r.get("sort")?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(v)
    }

    pub fn add_target(
        &self,
        label: &str,
        category: &str,
        mode: &str,
        amount: f64,
        unit: &str,
    ) -> Result<i64> {
        validate_target(label, mode, amount, unit)?;
        let c = self.c();
        c.execute(
            "INSERT INTO targets (label, category, mode, amount, unit, sort)
             VALUES (?1, ?2, ?3, ?4, ?5, (SELECT COALESCE(MAX(sort), 0) + 1 FROM targets))",
            params![label.trim(), category, mode, amount, unit],
        )?;
        Ok(c.last_insert_rowid())
    }

    pub fn update_target(
        &self,
        id: i64,
        label: &str,
        category: &str,
        mode: &str,
        amount: f64,
        unit: &str,
    ) -> Result<()> {
        validate_target(label, mode, amount, unit)?;
        self.c().execute(
            "UPDATE targets SET label = ?1, category = ?2, mode = ?3, amount = ?4, unit = ?5 WHERE id = ?6",
            params![label.trim(), category, mode, amount, unit, id],
        )?;
        Ok(())
    }

    pub fn delete_target(&self, id: i64) -> Result<()> {
        self.c()
            .execute("DELETE FROM targets WHERE id = ?1", [id])?;
        Ok(())
    }
}
