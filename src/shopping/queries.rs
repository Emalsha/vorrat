//! Queries for the shopping list and dismissed suggestions.

use std::collections::HashSet;

use anyhow::{bail, Result};
use rusqlite::params;
use serde::Serialize;

use crate::db::{now, Store};

#[derive(Debug, Clone, Serialize)]
pub struct ShoppingEntry {
    pub id: i64,
    pub label: String,
    pub item_id: Option<i64>,
    pub qty: Option<f64>,
    pub done: bool,
    pub added_on: String,
    pub item_name: Option<String>,
}

impl Store {
    pub fn shopping(&self) -> Result<Vec<ShoppingEntry>> {
        let c = self.r();
        let mut st = c.prepare_cached(
            "SELECT s.id, s.label, s.item_id, s.qty, s.done, s.added_on, i.name AS item_name
             FROM shopping s LEFT JOIN items i ON i.id = s.item_id ORDER BY s.done, s.added_on, s.id",
        )?;
        let v = st
            .query_map([], |r| {
                Ok(ShoppingEntry {
                    id: r.get("id")?,
                    label: r.get("label")?,
                    item_id: r.get("item_id")?,
                    qty: r.get("qty")?,
                    done: r.get::<_, i64>("done")? != 0,
                    added_on: r.get("added_on")?,
                    item_name: r.get("item_name")?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(v)
    }

    pub fn add_shopping(&self, label: &str, item_id: Option<i64>, qty: Option<f64>) -> Result<i64> {
        let label = label.trim();
        if label.is_empty() {
            bail!("label is empty");
        }
        let c = self.c();
        c.execute(
            "INSERT INTO shopping (label, item_id, qty, added_on) VALUES (?1, ?2, ?3, ?4)",
            params![label, item_id, qty, now()],
        )?;
        Ok(c.last_insert_rowid())
    }

    pub fn toggle_shopping(&self, id: i64) -> Result<()> {
        self.c()
            .execute("UPDATE shopping SET done = 1 - done WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn delete_shopping(&self, id: i64) -> Result<()> {
        self.c()
            .execute("DELETE FROM shopping WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn clear_shopping_done(&self) -> Result<()> {
        self.c()
            .execute("DELETE FROM shopping WHERE done = 1", [])?;
        Ok(())
    }

    pub fn dismiss(&self, key: &str, until: &str) -> Result<()> {
        self.c().execute(
            "INSERT INTO dismissed VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET until = excluded.until",
            params![key, until],
        )?;
        Ok(())
    }

    pub fn dismissed(&self, today: &str) -> Result<HashSet<String>> {
        let c = self.r();
        let mut st = c.prepare_cached("SELECT key FROM dismissed WHERE until > ?1")?;
        let v = st
            .query_map([today], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(v)
    }
}
