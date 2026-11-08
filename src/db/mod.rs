//! The SQLite store: one writer and a few pooled read-only connections (WAL
//! mode), settings, backup and export. The schema and its migrations are in
//! `schema.rs`. Each feature adds its own queries in its `queries.rs` as
//! another `impl Store` block, so this module names no feature.

pub mod schema;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use anyhow::{bail, Context, Result};
use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};

/// The tables a JSON export contains: the household's data, not bookkeeping.
pub const EXPORT_TABLES: &[&str] = &[
    "settings",
    "members",
    "locations",
    "items",
    "doses",
    "barcodes",
    "batches",
    "attachments",
    "targets",
    "shopping",
    "events",
];

pub fn now() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// Thread-safe handle on the database: one connection for writes, and a few
/// read-only ones so page loads run in parallel (SQLite WAL mode lets readers
/// work while a write is in progress). An in-memory database has no extra
/// readers; everything goes through the one connection.
pub struct Store {
    conn: Mutex<Connection>,
    readers: Vec<Mutex<Connection>>,
    next_reader: std::sync::atomic::AtomicUsize,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
        let mut store = Self::init(conn)?;
        let n = std::thread::available_parallelism()
            .map_or(2, |n| n.get())
            .clamp(2, 8);
        for _ in 0..n {
            let r = Connection::open_with_flags(
                path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            r.busy_timeout(std::time::Duration::from_secs(5))?;
            store.readers.push(Mutex::new(r));
        }
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let store = Self {
            conn: Mutex::new(conn),
            readers: Vec::new(),
            next_reader: std::sync::atomic::AtomicUsize::new(0),
        };
        store.migrate()?;
        Ok(store)
    }

    /// A connection for a read-only query: a free reader if there is one.
    pub(crate) fn r(&self) -> MutexGuard<'_, Connection> {
        if self.readers.is_empty() {
            return self.c();
        }
        let start = self
            .next_reader
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        for i in 0..self.readers.len() {
            if let Ok(g) = self.readers[(start + i) % self.readers.len()].try_lock() {
                return g;
            }
        }
        let m = &self.readers[start % self.readers.len()];
        m.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The writer connection. A panic while holding the lock leaves SQLite itself
    /// consistent (statements are atomic), so a poisoned lock is safe to reuse.
    pub(crate) fn c(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Every row of one of the household's tables as JSON objects, for export.
    pub fn dump_table(
        &self,
        table: &str,
    ) -> Result<Vec<serde_json::Map<String, serde_json::Value>>> {
        use rusqlite::types::ValueRef;
        if !EXPORT_TABLES.contains(&table) {
            bail!("not an exportable table: {table}");
        }
        let c = self.r();
        let mut st = c.prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))?;
        let cols: Vec<String> = st.column_names().iter().map(|c| c.to_string()).collect();
        let rows = st.query_map([], |r| {
            let mut m = serde_json::Map::new();
            for (i, name) in cols.iter().enumerate() {
                let v = match r.get_ref(i)? {
                    ValueRef::Null => serde_json::Value::Null,
                    ValueRef::Integer(n) => n.into(),
                    ValueRef::Real(f) => f.into(),
                    ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned().into(),
                    ValueRef::Blob(_) => serde_json::Value::Null,
                };
                m.insert(name.clone(), v);
            }
            Ok(m)
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Remove all stock and history, keeping locations, plan targets and
    /// settings. Only for the demo reset.
    pub fn wipe_stock(&self) -> Result<()> {
        let mut c = self.c();
        let tx = c.transaction()?;
        tx.execute_batch(
            "DELETE FROM attachments; DELETE FROM barcodes; DELETE FROM batches;
             DELETE FROM shopping; DELETE FROM events; DELETE FROM notified;
             DELETE FROM notified_low; DELETE FROM undo; DELETE FROM dismissed;
             DELETE FROM notified_dose; DELETE FROM doses; DELETE FROM items;",
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Consistent copy of the live database to `dest` (SQLite online backup).
    pub fn backup_to(&self, dest: &Path) -> Result<()> {
        let c = self.c();
        let mut out = Connection::open(dest)?;
        let b = rusqlite::backup::Backup::new(&c, &mut out)?;
        b.run_to_completion(256, std::time::Duration::from_millis(0), None)?;
        Ok(())
    }

    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .c()
            .query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .c()
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn setting_int(&self, key: &str, default: i64) -> i64 {
        self.setting(key)
            .ok()
            .flatten()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(default)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.c().execute(
            "INSERT INTO settings VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}
