//! Queries for medicine doses, reminders and checklist presets.

use std::collections::HashSet;

use anyhow::{bail, Result};
use chrono::NaiveDate;
use rusqlite::{params, OptionalExtension, Row};
use serde::Serialize;

use super::rules;
use crate::db::{now, Store};
use crate::stock::queries::NewItem;

/// The form a medicine comes in. Empty is allowed.
pub const MED_FORMS: &[&str] = &[
    "tablet",
    "capsule",
    "liquid",
    "drops",
    "inhaler",
    "injection",
    "patch",
    "cream",
    "other form",
];

/// A daily dose: what makes an item a medicine someone takes.
///
/// - `member_id`: the person or pet it is for; `None` when not set or they were removed.
/// - `form`: one of [`MED_FORMS`], or empty.
/// - `strength`: as written on the pack ("400 mg"); free text.
/// - `per_day`: in the item's unit.
/// - `renew_on`: when the prescription needs renewing.
#[derive(Debug, Clone, Serialize)]
pub struct Dose {
    pub item_id: i64,
    pub member_id: Option<i64>,
    pub form: String,
    pub strength: String,
    pub per_day: f64,
    pub renew_on: Option<String>,
}

impl Dose {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            item_id: r.get("item_id")?,
            member_id: r.get("member_id")?,
            form: r.get("form")?,
            strength: r.get("strength")?,
            per_day: r.get("per_day")?,
            renew_on: r.get("renew_on")?,
        })
    }
}

impl Store {
    pub fn doses(&self) -> Result<Vec<Dose>> {
        let c = self.r();
        let mut st = c.prepare(
            "SELECT d.item_id, d.member_id, d.form, d.strength, d.per_day, d.renew_on
             FROM doses d JOIN items i ON i.id = d.item_id ORDER BY i.name COLLATE NOCASE",
        )?;
        let rows = st.query_map([], Dose::from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn dose(&self, item_id: i64) -> Result<Option<Dose>> {
        Ok(self
            .r()
            .query_row(
                "SELECT item_id, member_id, form, strength, per_day, renew_on FROM doses WHERE item_id = ?1",
                [item_id],
                Dose::from_row,
            )
            .optional()?)
    }

    /// Add or change the daily dose of an item, which makes it a medicine.
    /// A new dose or date starts the reminders afresh.
    pub fn set_dose(
        &self,
        item_id: i64,
        member_id: Option<i64>,
        form: &str,
        strength: &str,
        per_day: f64,
        renew_on: Option<NaiveDate>,
    ) -> Result<()> {
        let (form, strength) = self.validate_dose(member_id, form, strength, per_day)?;
        let c = self.c();
        let n = c.execute(
            "INSERT INTO doses (item_id, member_id, form, strength, per_day, renew_on)
             SELECT id, ?2, ?3, ?4, ?5, ?6 FROM items WHERE id = ?1
             ON CONFLICT (item_id) DO UPDATE SET member_id = excluded.member_id,
                 form = excluded.form, strength = excluded.strength,
                 per_day = excluded.per_day, renew_on = excluded.renew_on",
            params![
                item_id,
                member_id,
                form,
                strength,
                per_day,
                renew_on.map(|d| d.to_string())
            ],
        )?;
        if n == 0 {
            bail!("no such item");
        }
        c.execute("DELETE FROM notified_dose WHERE item_id = ?1", [item_id])?;
        Ok(())
    }

    pub fn delete_dose(&self, item_id: i64) -> Result<()> {
        let c = self.c();
        c.execute("DELETE FROM doses WHERE item_id = ?1", [item_id])?;
        c.execute("DELETE FROM notified_dose WHERE item_id = ?1", [item_id])?;
        Ok(())
    }

    /// A new medicine item with its dose, in one step.
    /// The dose is checked before the item exists, so a bad form adds nothing.
    #[allow(clippy::too_many_arguments)]
    pub fn add_medicine(
        &self,
        name: &str,
        unit: &str,
        member_id: Option<i64>,
        form: &str,
        strength: &str,
        per_day: f64,
        renew_on: Option<NaiveDate>,
    ) -> Result<i64> {
        self.validate_dose(member_id, form, strength, per_day)?;
        let id = self.add_item(NewItem {
            name: name.into(),
            kind: Some("supply".into()),
            category: Some("medicine".into()),
            unit: Some(unit.into()),
            ..Default::default()
        })?;
        self.set_dose(id, member_id, form, strength, per_day, renew_on)?;
        Ok(id)
    }

    fn validate_dose(
        &self,
        member_id: Option<i64>,
        form: &str,
        strength: &str,
        per_day: f64,
    ) -> Result<(String, String)> {
        let form = form.trim();
        if !form.is_empty() && !MED_FORMS.contains(&form) {
            bail!("unknown form {form:?}");
        }
        let strength = strength.trim();
        if strength.chars().count() > 40 {
            bail!("strength is too long");
        }
        if !(per_day.is_finite() && per_day > 0.0 && per_day <= 1000.0) {
            bail!("the daily dose must be more than 0");
        }
        if let Some(m) = member_id {
            let known: bool = self.r().query_row(
                "SELECT EXISTS(SELECT 1 FROM members WHERE id = ?1)",
                [m],
                |r| r.get(0),
            )?;
            if !known {
                bail!("no such person or pet");
            }
        }
        Ok((form.to_string(), strength.to_string()))
    }

    /// Reminder marks already sent for medicines: (item, mark).
    pub fn dose_marks(&self) -> Result<HashSet<(i64, String)>> {
        let c = self.r();
        let mut st = c.prepare("SELECT item_id, mark FROM notified_dose")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn mark_dose(&self, item_id: i64, mark: &str) -> Result<()> {
        self.c().execute(
            "INSERT OR IGNORE INTO notified_dose VALUES (?1, ?2, ?3)",
            params![item_id, mark, now()],
        )?;
        Ok(())
    }

    pub fn clear_dose_mark(&self, item_id: i64, mark: &str) -> Result<()> {
        self.c().execute(
            "DELETE FROM notified_dose WHERE item_id = ?1 AND mark = ?2",
            params![item_id, mark],
        )?;
        Ok(())
    }

    /// Let an item stand for a checklist entry, or for none.
    pub fn set_preset(&self, item_id: i64, key: Option<&str>) -> Result<()> {
        if let Some(k) = key {
            if rules::find(k).is_none() {
                bail!("unknown checklist entry {k:?}");
            }
        }
        let n = self.c().execute(
            "UPDATE items SET preset = ?2 WHERE id = ?1",
            params![item_id, key],
        )?;
        if n == 0 {
            bail!("no such item");
        }
        Ok(())
    }
}
