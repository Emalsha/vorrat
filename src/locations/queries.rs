//! Queries for locations.

use std::collections::{HashMap, HashSet};

use anyhow::{bail, Result};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::db::Store;

/// A place where things are kept.
///
/// - `path`: breadcrumb, e.g. "Kitchen › Pantry".
#[derive(Debug, Clone, Serialize)]
pub struct Location {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub path: String,
}

impl Store {
    /// All locations with a breadcrumb path, sorted by path.
    /// Walking up the tree guards against cycles.
    pub fn locations(&self) -> Result<Vec<Location>> {
        let rows: Vec<(i64, String, Option<i64>)> = {
            let c = self.r();
            let mut st = c.prepare_cached("SELECT id, name, parent_id FROM locations")?;
            let v = st
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<rusqlite::Result<_>>()?;
            v
        };
        let by_id: HashMap<i64, (String, Option<i64>)> = rows
            .iter()
            .map(|(id, n, p)| (*id, (n.clone(), *p)))
            .collect();
        let mut out: Vec<Location> = rows
            .iter()
            .map(|(id, name, parent)| {
                let mut parts = vec![name.clone()];
                let mut seen = HashSet::from([*id]);
                let mut cur = *parent;
                while let Some(pid) = cur {
                    if !seen.insert(pid) {
                        break;
                    }
                    match by_id.get(&pid) {
                        Some((n, p)) => {
                            parts.push(n.clone());
                            cur = *p;
                        }
                        None => break,
                    }
                }
                parts.reverse();
                Location {
                    id: *id,
                    name: name.clone(),
                    parent_id: *parent,
                    path: parts.join(" › "),
                }
            })
            .collect();
        out.sort_by_key(|l| l.path.to_lowercase());
        Ok(out)
    }

    pub fn add_location(&self, name: &str, parent_id: Option<i64>) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() {
            bail!("location name is empty");
        }
        let c = self.c();
        c.execute(
            "INSERT INTO locations (name, parent_id) VALUES (?1, ?2)",
            params![name, parent_id],
        )?;
        Ok(c.last_insert_rowid())
    }

    /// A location can't sit inside itself or inside one of its own children.
    pub fn rename_location(&self, id: i64, name: &str, parent_id: Option<i64>) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            bail!("location name is empty");
        }
        if let Some(p) = parent_id {
            let mut cur = Some(p);
            let mut hops = 0;
            while let Some(c) = cur {
                if c == id || hops > 100 {
                    bail!("a location can't be inside itself");
                }
                cur = self
                    .c()
                    .query_row("SELECT parent_id FROM locations WHERE id = ?1", [c], |r| {
                        r.get(0)
                    })
                    .optional()?
                    .flatten();
                hops += 1;
            }
        }
        self.c().execute(
            "UPDATE locations SET name = ?1, parent_id = ?2 WHERE id = ?3",
            params![name, parent_id, id],
        )?;
        Ok(())
    }

    /// Distinct items stored per location.
    pub fn location_counts(&self) -> Result<HashMap<i64, i64>> {
        let c = self.r();
        let mut st = c.prepare_cached("SELECT location_id, COUNT(DISTINCT item_id) FROM batches WHERE location_id IS NOT NULL GROUP BY location_id")?;
        let v = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(v)
    }

    pub fn delete_location(&self, id: i64) -> Result<()> {
        self.c()
            .execute("DELETE FROM locations WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Per item in a category: its usual location, else where most of its stock is.
    pub fn category_locations(&self, category: &str) -> Result<Vec<Option<i64>>> {
        let c = self.r();
        let mut st = c.prepare_cached(
            "SELECT COALESCE(i.default_location_id,
                    (SELECT b.location_id FROM batches b WHERE b.item_id = i.id AND b.location_id IS NOT NULL
                     GROUP BY b.location_id ORDER BY SUM(b.qty) DESC LIMIT 1))
             FROM items i WHERE i.category = ?1",
        )?;
        let v = st
            .query_map([category], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(v)
    }
}
