//! What has already been notified, so a reminder is sent once.

use std::collections::HashSet;

use anyhow::Result;
use rusqlite::params;

use crate::db::{now, Store};

impl Store {
    pub fn notified_thresholds(&self, batch_id: i64) -> Result<HashSet<i64>> {
        let c = self.r();
        let mut st = c.prepare_cached("SELECT threshold FROM notified WHERE batch_id = ?1")?;
        let v = st
            .query_map([batch_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(v)
    }

    pub fn mark_notified(&self, batch_id: i64, thresholds: &[i64]) -> Result<()> {
        let c = self.c();
        for t in thresholds {
            c.execute(
                "INSERT OR IGNORE INTO notified VALUES (?1, ?2, ?3)",
                params![batch_id, t, now()],
            )?;
        }
        Ok(())
    }

    pub fn low_notified(&self) -> Result<HashSet<i64>> {
        let c = self.r();
        let mut st = c.prepare_cached("SELECT item_id FROM notified_low")?;
        let v = st
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(v)
    }

    pub fn mark_low(&self, item_id: i64) -> Result<()> {
        self.c().execute(
            "INSERT OR IGNORE INTO notified_low VALUES (?1, ?2)",
            params![item_id, now()],
        )?;
        Ok(())
    }

    pub fn clear_low(&self, item_id: i64) -> Result<()> {
        self.c()
            .execute("DELETE FROM notified_low WHERE item_id = ?1", [item_id])?;
        Ok(())
    }
}
