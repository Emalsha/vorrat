//! Queries for people and pets.

use anyhow::{bail, Result};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::db::Store;

/// Age groups for people and kinds of pet, in the order the page shows them.
pub const PERSON_TYPES: &[&str] = &["baby", "child", "teen", "adult", "older adult"];

pub const PET_TYPES: &[&str] = &["dog", "cat", "small animal", "bird", "other pet"];

/// Needs a person can have. They will drive suggestions; for now they are noted.
pub const NEEDS: &[&str] = &[
    "pregnant or breastfeeding",
    "vegetarian",
    "vegan",
    "halal",
    "kosher",
    "gluten-free",
    "allergies",
    "care needs",
    "glasses",
    "hearing aid",
];

/// A person or pet in the household.
///
/// - `kind`: `person` or `pet`.
/// - `type`: one of [`PERSON_TYPES`] or [`PET_TYPES`].
/// - `needs`: entries of [`NEEDS`].
#[derive(Debug, Clone, Serialize)]
pub struct Member {
    pub id: i64,
    pub kind: String,
    pub r#type: String,
    pub name: Option<String>,
    pub needs: Vec<String>,
}

fn validate_member(
    kind: &str,
    r#type: &str,
    name: &str,
    needs: &[String],
) -> Result<(Option<String>, String)> {
    let types = match kind {
        "person" => PERSON_TYPES,
        "pet" => PET_TYPES,
        _ => bail!("unknown kind {kind:?}"),
    };
    if !types.contains(&r#type) {
        bail!("unknown type {type:?}", type = r#type);
    }
    let name = name.trim();
    if name.chars().count() > 60 {
        bail!("name is too long");
    }
    if kind == "pet" && !needs.is_empty() {
        bail!("needs are for people");
    }
    if let Some(n) = needs.iter().find(|n| !NEEDS.contains(&n.as_str())) {
        bail!("unknown need {n:?}");
    }
    let mut needs: Vec<&str> = NEEDS
        .iter()
        .copied()
        .filter(|n| needs.iter().any(|x| x == n))
        .collect();
    needs.dedup();
    Ok((
        (!name.is_empty()).then(|| name.to_string()),
        needs.join(","),
    ))
}

impl Store {
    pub fn members(&self) -> Result<Vec<Member>> {
        let c = self.r();
        let mut st = c.prepare(
            "SELECT id, kind, type, name, needs FROM members ORDER BY kind DESC, sort, id",
        )?;
        let rows = st.query_map([], |r| {
            let needs: String = r.get(4)?;
            Ok(Member {
                id: r.get(0)?,
                kind: r.get(1)?,
                r#type: r.get(2)?,
                name: r.get(3)?,
                needs: needs
                    .split(',')
                    .filter(|n| !n.is_empty())
                    .map(String::from)
                    .collect(),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// (people, pets) in the household profile.
    pub fn household_counts(&self) -> Result<(i64, i64)> {
        Ok(self.r().query_row(
            "SELECT COUNT(*) FILTER (WHERE kind = 'person'), COUNT(*) FILTER (WHERE kind = 'pet') FROM members",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?)
    }

    /// (adults, children): adults and older adults; babies, children and teens.
    pub fn age_counts(&self) -> Result<(i64, i64)> {
        Ok(self.r().query_row(
            "SELECT COUNT(*) FILTER (WHERE type IN ('adult', 'older adult')),
                    COUNT(*) FILTER (WHERE type IN ('baby', 'child', 'teen'))
             FROM members WHERE kind = 'person'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?)
    }

    pub fn add_member(
        &self,
        kind: &str,
        r#type: &str,
        name: &str,
        needs: &[String],
    ) -> Result<i64> {
        let (name, needs) = validate_member(kind, r#type, name, needs)?;
        if self.members()?.len() >= 50 {
            bail!("a household can have at most 50 people and pets");
        }
        let c = self.c();
        c.execute(
            "INSERT INTO members (kind, type, name, needs, sort)
             VALUES (?1, ?2, ?3, ?4, (SELECT COALESCE(MAX(sort), 0) + 1 FROM members))",
            params![kind, r#type, name, needs],
        )?;
        let id = c.last_insert_rowid();
        drop(c);
        self.sync_people()?;
        Ok(id)
    }

    pub fn update_member(&self, id: i64, r#type: &str, name: &str, needs: &[String]) -> Result<()> {
        let kind: String = self
            .r()
            .query_row("SELECT kind FROM members WHERE id = ?1", [id], |r| r.get(0))
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("no such person or pet"))?;
        let (name, needs) = validate_member(&kind, r#type, name, needs)?;
        self.c().execute(
            "UPDATE members SET type = ?1, name = ?2, needs = ?3 WHERE id = ?4",
            params![r#type, name, needs, id],
        )?;
        Ok(())
    }

    pub fn delete_member(&self, id: i64) -> Result<()> {
        self.c()
            .execute("DELETE FROM members WHERE id = ?1", [id])?;
        self.sync_people()
    }

    /// Keeps the old "people" setting equal to the number of people, for the
    /// API and exports that still read it.
    fn sync_people(&self) -> Result<()> {
        let (people, _) = self.household_counts()?;
        self.set_setting("people", &people.to_string())
    }
}
