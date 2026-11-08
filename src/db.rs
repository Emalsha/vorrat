//! SQLite store: one file, schema migrations via `PRAGMA user_version`.
//!
//! Schema v4. Databases made by earlier versions (including the
//! Python prototype) are upgraded on start. New migrations append to
//! [`SCHEMA`]; never edit an existing step.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use anyhow::{bail, Context, Result};
use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

pub const KINDS: &[&str] = &["food", "supply", "document", "thing"];
pub const UNITS: &[&str] = &["pcs", "g", "kg", "ml", "L"];

/// Categories double as the link between stock and the emergency plan (targets).
pub const CATEGORIES: &[&str] = &[
    "water",
    "drinks",
    "grains",
    "vegetables",
    "fruit",
    "dairy",
    "protein",
    "fats",
    "ready meals",
    "snacks",
    "spices",
    "baby",
    "pet",
    "medicine",
    "hygiene",
    "cleaning",
    "batteries",
    "light",
    "tools",
    "first aid",
    "documents",
    "cash",
    "cooking",
    "radio",
    "warmth",
    "other",
];

/// The schema, one entry per version; `PRAGMA user_version` says how many have
/// run. Never change an entry, only add one.
///
/// - v1: the first tables.
/// - v2: extra barcodes, open-days shelf life, Nutri-Score, structured expiry on
///   purchase events, undo, dismissed suggestions.
/// - v3: the household profile. People and pets, one row each; the plan scales
///   from it instead of the single "people" setting. Existing households get one
///   adult per person they had set.
/// - v4: medicine and essentials. A medicine is an item with a daily dose for one
///   person or pet; reminders about it are sent once per mark. Items can stand
///   for an entry of the first-aid or essentials checklist.
const SCHEMA: &[&str] = &[
    r#"
    CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE locations (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        parent_id INTEGER REFERENCES locations(id) ON DELETE SET NULL
    );
    CREATE TABLE items (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        kind TEXT NOT NULL DEFAULT 'food',
        barcode TEXT UNIQUE,
        brand TEXT,
        category TEXT,
        unit TEXT NOT NULL DEFAULT 'pcs',
        size REAL,
        size_unit TEXT,
        min_qty REAL NOT NULL DEFAULT 0,
        default_location_id INTEGER REFERENCES locations(id) ON DELETE SET NULL,
        notes TEXT,
        image TEXT,
        created_at TEXT NOT NULL
    );
    CREATE TABLE batches (
        id INTEGER PRIMARY KEY,
        item_id INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
        location_id INTEGER REFERENCES locations(id) ON DELETE SET NULL,
        qty REAL NOT NULL,
        expires_on TEXT,
        added_on TEXT NOT NULL,
        opened_on TEXT,
        price REAL,
        note TEXT
    );
    CREATE INDEX batches_item ON batches(item_id);
    CREATE TABLE attachments (
        id INTEGER PRIMARY KEY,
        item_id INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
        filename TEXT NOT NULL,
        stored TEXT NOT NULL,
        mime TEXT,
        size INTEGER,
        added_on TEXT NOT NULL
    );
    CREATE TABLE targets (
        id INTEGER PRIMARY KEY,
        label TEXT NOT NULL,
        category TEXT NOT NULL,
        mode TEXT NOT NULL DEFAULT 'per_person_day',
        amount REAL NOT NULL,
        unit TEXT NOT NULL,
        sort INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE shopping (
        id INTEGER PRIMARY KEY,
        label TEXT NOT NULL,
        item_id INTEGER REFERENCES items(id) ON DELETE CASCADE,
        qty REAL,
        done INTEGER NOT NULL DEFAULT 0,
        added_on TEXT NOT NULL
    );
    CREATE TABLE events (
        id INTEGER PRIMARY KEY,
        ts TEXT NOT NULL,
        item_id INTEGER REFERENCES items(id) ON DELETE CASCADE,
        action TEXT NOT NULL,
        qty REAL,
        price REAL,
        detail TEXT
    );
    CREATE INDEX events_item ON events(item_id, ts);
    CREATE TABLE notified (
        batch_id INTEGER NOT NULL REFERENCES batches(id) ON DELETE CASCADE,
        threshold INTEGER NOT NULL,
        sent_at TEXT NOT NULL,
        PRIMARY KEY (batch_id, threshold)
    );
    CREATE TABLE notified_low (
        item_id INTEGER PRIMARY KEY REFERENCES items(id) ON DELETE CASCADE,
        sent_at TEXT NOT NULL
    );
    "#,
    r#"
    ALTER TABLE items ADD COLUMN open_days INTEGER;
    ALTER TABLE items ADD COLUMN nutriscore TEXT;
    CREATE TABLE barcodes (
        code TEXT PRIMARY KEY,
        item_id INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE
    );
    ALTER TABLE events ADD COLUMN expires_on TEXT;
    UPDATE events SET expires_on = substr(detail, 9)
        WHERE action = 'add' AND detail LIKE 'expires ____-__-__';
    CREATE TABLE undo (
        token TEXT PRIMARY KEY,
        item_id INTEGER NOT NULL,
        label TEXT NOT NULL,
        payload TEXT NOT NULL,
        created_at TEXT NOT NULL
    );
    CREATE TABLE dismissed (key TEXT PRIMARY KEY, until TEXT NOT NULL);
    "#,
    r#"
    CREATE TABLE members (
        id INTEGER PRIMARY KEY,
        kind TEXT NOT NULL CHECK (kind IN ('person', 'pet')),
        type TEXT NOT NULL,
        name TEXT,
        needs TEXT NOT NULL DEFAULT '',
        sort INTEGER NOT NULL DEFAULT 0
    );
    INSERT INTO members (kind, type, sort)
        WITH RECURSIVE n(i) AS (
            SELECT 1 WHERE EXISTS (SELECT 1 FROM settings WHERE key = 'people')
            UNION ALL
            SELECT i + 1 FROM n
            WHERE i < (SELECT MIN(MAX(CAST(value AS INTEGER), 1), 50) FROM settings WHERE key = 'people')
        )
        SELECT 'person', 'adult', i FROM n;
    "#,
    r#"
    CREATE TABLE doses (
        item_id INTEGER PRIMARY KEY REFERENCES items(id) ON DELETE CASCADE,
        member_id INTEGER REFERENCES members(id) ON DELETE SET NULL,
        form TEXT NOT NULL DEFAULT '',
        strength TEXT NOT NULL DEFAULT '',
        per_day REAL NOT NULL CHECK (per_day > 0),
        renew_on TEXT
    );
    CREATE TABLE notified_dose (
        item_id INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
        mark TEXT NOT NULL,
        sent_at TEXT NOT NULL,
        PRIMARY KEY (item_id, mark)
    );
    ALTER TABLE items ADD COLUMN preset TEXT;
    "#,
];

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
/// How a plan target scales with the household.
pub const TARGET_MODES: &[&str] = &[
    "per_person_day",
    "per_person",
    "per_pet_day",
    "per_pet",
    "fixed",
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
            item_id: r.get(0)?,
            member_id: r.get(1)?,
            form: r.get(2)?,
            strength: r.get(3)?,
            per_day: r.get(4)?,
            renew_on: r.get(5)?,
        })
    }
}

/// Seeded once on a fresh DB. Per person per day, from the German civil
/// protection office (BBK) 10-day checklist divided by 10. Editable in the app.
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

pub const DEFAULT_TARGETS: &[(&str, &str, &str, f64, &str)] = &[
    (
        "Drinking water & drinks",
        "water",
        "per_person_day",
        2.0,
        "L",
    ),
    (
        "Grains, bread, pasta, rice, potatoes",
        "grains",
        "per_person_day",
        0.35,
        "kg",
    ),
    (
        "Vegetables & pulses",
        "vegetables",
        "per_person_day",
        0.4,
        "kg",
    ),
    ("Fruit & nuts", "fruit", "per_person_day", 0.25, "kg"),
    (
        "Milk & dairy (long-life)",
        "dairy",
        "per_person_day",
        0.26,
        "kg",
    ),
    ("Fish, meat, eggs", "protein", "per_person_day", 0.15, "kg"),
    ("Fats & oils", "fats", "per_person_day", 0.036, "kg"),
    ("Batteries", "batteries", "fixed", 12.0, "pcs"),
    ("Torch / candles", "light", "fixed", 2.0, "pcs"),
    ("First-aid kit", "first aid", "fixed", 1.0, "pcs"),
];

pub const DEFAULT_LOCATIONS: &[&str] = &[
    "Kitchen",
    "Pantry",
    "Fridge",
    "Freezer",
    "Cellar",
    "Emergency box",
    "Documents",
];
pub const DEFAULT_SETTINGS: &[(&str, &str)] = &[("people", "2"), ("days", "14")];

pub fn now() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

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

/// Something the household keeps, with its stock.
///
/// - `preset`: the checklist entry this item stands for (see `essentials.rs`).
/// - `total`: sum of all batches.
#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub barcode: Option<String>,
    pub brand: Option<String>,
    pub category: Option<String>,
    pub unit: String,
    pub size: Option<f64>,
    pub size_unit: Option<String>,
    pub min_qty: f64,
    pub default_location_id: Option<i64>,
    pub notes: Option<String>,
    pub image: Option<String>,
    pub open_days: Option<i64>,
    pub nutriscore: Option<String>,
    pub preset: Option<String>,
    pub created_at: String,
    pub total: f64,
    pub next_expiry: Option<String>,
    pub batch_count: i64,
}

impl Item {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get("id")?,
            name: r.get("name")?,
            kind: r.get("kind")?,
            barcode: r.get("barcode")?,
            brand: r.get("brand")?,
            category: r.get("category")?,
            unit: r.get("unit")?,
            size: r.get("size")?,
            size_unit: r.get("size_unit")?,
            min_qty: r.get("min_qty")?,
            default_location_id: r.get("default_location_id")?,
            notes: r.get("notes")?,
            image: r.get("image")?,
            open_days: r.get("open_days")?,
            nutriscore: r.get("nutriscore")?,
            preset: r.get("preset")?,
            created_at: r.get("created_at")?,
            total: r.get("total")?,
            next_expiry: r.get("next_expiry")?,
            batch_count: r.get("batch_count")?,
        })
    }

    pub fn is_low(&self) -> bool {
        self.min_qty > 0.0 && self.total < self.min_qty
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Batch {
    pub id: i64,
    pub item_id: i64,
    pub location_id: Option<i64>,
    pub qty: f64,
    pub expires_on: Option<String>,
    pub added_on: String,
    pub opened_on: Option<String>,
    pub price: Option<f64>,
    pub note: Option<String>,
    pub item_name: String,
    pub kind: String,
    pub unit: String,
    pub category: Option<String>,
    pub image: Option<String>,
    pub location_name: Option<String>,
}

impl Batch {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get("id")?,
            item_id: r.get("item_id")?,
            location_id: r.get("location_id")?,
            qty: r.get("qty")?,
            expires_on: r.get("expires_on")?,
            added_on: r.get("added_on")?,
            opened_on: r.get("opened_on")?,
            price: r.get("price")?,
            note: r.get("note")?,
            item_name: r.get("item_name")?,
            kind: r.get("kind")?,
            unit: r.get("unit")?,
            category: r.get("category")?,
            image: r.get("image")?,
            location_name: r.get("location_name")?,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Attachment {
    pub id: i64,
    pub item_id: i64,
    pub filename: String,
    #[serde(skip)]
    pub stored: String,
    pub mime: Option<String>,
    pub size: Option<i64>,
}

impl Attachment {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            item_id: r.get(1)?,
            filename: r.get(2)?,
            stored: r.get(3)?,
            mime: r.get(4)?,
            size: r.get(5)?,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Price {
    pub ts: String,
    pub qty: Option<f64>,
    pub price: f64,
    pub unit_price: Option<f64>,
}

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

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub id: i64,
    pub ts: String,
    pub item_id: Option<i64>,
    pub action: String,
    pub qty: Option<f64>,
    pub price: Option<f64>,
    pub detail: Option<String>,
    pub item_name: Option<String>,
}

/// Fields for a new item. Everything but the name is optional.
///
/// - `image`: file name in the data `files/` folder (a picture from the barcode lookup).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct NewItem {
    pub name: String,
    pub kind: Option<String>,
    pub barcode: Option<String>,
    pub brand: Option<String>,
    pub category: Option<String>,
    pub unit: Option<String>,
    pub size: Option<f64>,
    pub size_unit: Option<String>,
    pub min_qty: Option<f64>,
    pub default_location_id: Option<i64>,
    pub notes: Option<String>,
    pub open_days: Option<i64>,
    pub image: Option<String>,
    pub nutriscore: Option<String>,
}

fn blank_to_none(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

const ITEM_SELECT: &str =
    "SELECT i.*, COALESCE(SUM(b.qty), 0.0) AS total, MIN(b.expires_on) AS next_expiry,
       COUNT(b.id) AS batch_count
FROM items i LEFT JOIN batches b ON b.item_id = i.id";

const BATCH_SELECT: &str = "SELECT b.*, i.name AS item_name, i.kind, i.unit, i.category, i.image,
       l.name AS location_name
FROM batches b JOIN items i ON i.id = b.item_id
LEFT JOIN locations l ON l.id = b.location_id";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Name,
    Expiry,
    Recent,
}

impl Sort {
    pub fn parse(s: &str) -> Self {
        match s {
            "expiry" => Sort::Expiry,
            "recent" => Sort::Recent,
            _ => Sort::Name,
        }
    }
    fn sql(self) -> &'static str {
        match self {
            Sort::Name => "i.name COLLATE NOCASE",
            Sort::Expiry => "next_expiry IS NULL, next_expiry, i.name COLLATE NOCASE",
            Sort::Recent => "(SELECT MAX(ts) FROM events e WHERE e.item_id = i.id AND e.action = 'add') DESC, i.name COLLATE NOCASE",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ItemQuery<'a> {
    pub search: &'a str,
    pub kind: &'a str,
    pub location_id: Option<i64>,
    pub in_stock_only: bool,
    pub sort: Sort,
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
    fn r(&self) -> MutexGuard<'_, Connection> {
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
    fn c(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Runs the schema versions not yet applied. When settings did not exist yet
    /// as v3 ran, the household starts with two adults. Indexes are speed-ups, not
    /// a schema version, so older versions can still open the file.
    fn migrate(&self) -> Result<()> {
        let mut c = self.c();
        let ver: usize = c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? as usize;
        if ver > SCHEMA.len() {
            bail!(
                "database schema v{ver} is newer than this Vorrat (v{}); upgrade Vorrat",
                SCHEMA.len()
            );
        }
        let tx = c.transaction()?;
        for (i, sql) in SCHEMA.iter().enumerate().skip(ver) {
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        }
        if ver == 0 {
            for (k, v) in DEFAULT_SETTINGS {
                tx.execute("INSERT INTO settings VALUES (?1, ?2)", params![k, v])?;
            }
            for n in DEFAULT_LOCATIONS {
                tx.execute("INSERT INTO locations (name) VALUES (?1)", [n])?;
            }
            for (i, (label, cat, mode, amount, unit)) in DEFAULT_TARGETS.iter().enumerate() {
                tx.execute(
                    "INSERT INTO targets (label, category, mode, amount, unit, sort) VALUES (?1,?2,?3,?4,?5,?6)",
                    params![label, cat, mode, amount, unit, i as i64],
                )?;
            }
            tx.execute_batch(
                "INSERT INTO members (kind, type, sort) VALUES ('person', 'adult', 1), ('person', 'adult', 2);",
            )?;
        }
        tx.execute_batch(
            "CREATE INDEX IF NOT EXISTS events_ts ON events(ts);
             CREATE INDEX IF NOT EXISTS batches_expiry ON batches(expires_on);",
        )?;
        tx.commit()?;
        Ok(())
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
            if crate::essentials::find(k).is_none() {
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

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.c().execute(
            "INSERT INTO settings VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

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

    pub fn add_item(&self, f: NewItem) -> Result<i64> {
        let name = f.name.trim();
        if name.is_empty() {
            bail!("name is required");
        }
        let kind = blank_to_none(f.kind).unwrap_or_else(|| "food".into());
        if !KINDS.contains(&kind.as_str()) {
            bail!("unknown kind {kind:?}");
        }
        let unit = blank_to_none(f.unit).unwrap_or_else(|| "pcs".into());
        if !UNITS.contains(&unit.as_str()) {
            bail!("unknown unit {unit:?}");
        }
        let c = self.c();
        c.execute(
            "INSERT INTO items (name, kind, barcode, brand, category, unit, size, size_unit, min_qty,
                                default_location_id, notes, open_days, image, nutriscore, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                name,
                kind,
                blank_to_none(f.barcode),
                blank_to_none(f.brand),
                blank_to_none(f.category),
                unit,
                f.size,
                blank_to_none(f.size_unit),
                f.min_qty.unwrap_or(0.0).max(0.0),
                f.default_location_id,
                blank_to_none(f.notes),
                f.open_days,
                blank_to_none(f.image).filter(|n| crate::files::is_safe_name(n)),
                blank_to_none(f.nutriscore),
                now()
            ],
        )?;
        Ok(c.last_insert_rowid())
    }

    pub fn delete_item(&self, id: i64) -> Result<()> {
        self.c().execute("DELETE FROM items WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn set_min_qty(&self, id: i64, min_qty: f64) -> Result<()> {
        self.c().execute(
            "UPDATE items SET min_qty = ?1 WHERE id = ?2",
            params![min_qty.max(0.0), id],
        )?;
        self.log(Some(id), "edit", None, None, Some("minimum changed"), None)
    }

    pub fn item(&self, id: i64) -> Result<Option<Item>> {
        let c = self.r();
        Ok(c.query_row(
            &format!("{ITEM_SELECT} WHERE i.id = ?1 GROUP BY i.id"),
            [id],
            Item::from_row,
        )
        .optional()?)
    }

    /// The item's own barcode, or one linked to it later.
    pub fn item_by_barcode(&self, code: &str) -> Result<Option<Item>> {
        let code = code.trim();
        let c = self.r();
        Ok(c.query_row(
            &format!(
                "{ITEM_SELECT} WHERE i.barcode = ?1 OR i.id = (SELECT item_id FROM barcodes WHERE code = ?1) GROUP BY i.id"
            ),
            [code],
            Item::from_row,
        )
        .optional()?)
    }

    pub fn items(&self, q: &ItemQuery) -> Result<Vec<Item>> {
        let mut where_ = Vec::new();
        let mut args: Vec<rusqlite::types::Value> = Vec::new();
        let search = q.search.trim();
        if !search.is_empty() {
            let like = format!("%{search}%");
            where_.push(
                "(i.name LIKE ? OR i.brand LIKE ? OR i.barcode = ? OR i.category LIKE ? OR i.notes LIKE ?
                  OR i.id IN (SELECT item_id FROM barcodes WHERE code = ?))",
            );
            for v in [
                &like,
                &like,
                &search.to_string(),
                &like,
                &like,
                &search.to_string(),
            ] {
                args.push(v.clone().into());
            }
        }
        if !q.kind.is_empty() {
            where_.push("i.kind = ?");
            args.push(q.kind.to_string().into());
        }
        if let Some(l) = q.location_id {
            where_.push("i.id IN (SELECT item_id FROM batches WHERE location_id = ?)");
            args.push(l.into());
        }
        let mut sql = ITEM_SELECT.to_string();
        if !where_.is_empty() {
            sql += " WHERE ";
            sql += &where_.join(" AND ");
        }
        sql += " GROUP BY i.id";
        if q.in_stock_only {
            sql += " HAVING total > 0";
        }
        sql += " ORDER BY ";
        sql += q.sort.sql();
        let c = self.r();
        let mut st = c.prepare_cached(&sql)?;
        let v = st
            .query_map(rusqlite::params_from_iter(args), Item::from_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(v)
    }

    pub fn all_items(&self) -> Result<Vec<Item>> {
        self.items(&ItemQuery::default())
    }

    /// Save the edit form. Rejects a barcode that belongs to another item.
    pub fn update_item(&self, id: i64, f: NewItem) -> Result<()> {
        let name = f.name.trim();
        if name.is_empty() {
            bail!("name is required");
        }
        let kind = blank_to_none(f.kind).unwrap_or_else(|| "food".into());
        let unit = blank_to_none(f.unit).unwrap_or_else(|| "pcs".into());
        if !KINDS.contains(&kind.as_str()) || !UNITS.contains(&unit.as_str()) {
            bail!("unknown kind or unit");
        }
        let barcode = blank_to_none(f.barcode);
        if let Some(code) = &barcode {
            if let Some(other) = self.item_by_barcode(code)? {
                if other.id != id {
                    bail!("barcode already belongs to {}", other.name);
                }
            }
        }
        self.c().execute(
            "UPDATE items SET name = ?1, kind = ?2, barcode = ?3, brand = ?4, category = ?5, unit = ?6, size = ?7,
                    size_unit = ?8, min_qty = ?9, default_location_id = ?10, notes = ?11, open_days = ?12
             WHERE id = ?13",
            params![
                name, kind, barcode, blank_to_none(f.brand), blank_to_none(f.category), unit, f.size,
                blank_to_none(f.size_unit), f.min_qty.unwrap_or(0.0).max(0.0), f.default_location_id,
                blank_to_none(f.notes), f.open_days.filter(|d| *d > 0), id
            ],
        )?;
        self.log(Some(id), "edit", None, None, None, None)
    }

    pub fn set_image(&self, id: i64, image: Option<&str>) -> Result<()> {
        self.c().execute(
            "UPDATE items SET image = ?1 WHERE id = ?2",
            params![image, id],
        )?;
        Ok(())
    }

    /// Link another barcode to an item (another brand, a new pack design).
    /// Refused if the code already belongs to an item.
    pub fn link_barcode(&self, code: &str, item_id: i64) -> Result<()> {
        let code = code.trim();
        if code.is_empty() || code.len() > 64 {
            bail!("barcode is empty or too long");
        }
        if let Some(other) = self.item_by_barcode(code)? {
            bail!("barcode already belongs to {}", other.name);
        }
        self.c().execute(
            "INSERT INTO barcodes (code, item_id) VALUES (?1, ?2)",
            params![code, item_id],
        )?;
        self.log(
            Some(item_id),
            "edit",
            None,
            None,
            Some(&format!("barcode {code} linked")),
            None,
        )
    }

    pub fn extra_barcodes(&self, item_id: i64) -> Result<Vec<String>> {
        let c = self.r();
        let mut st =
            c.prepare_cached("SELECT code FROM barcodes WHERE item_id = ?1 ORDER BY code")?;
        let v = st
            .query_map([item_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(v)
    }

    pub fn unlink_barcode(&self, code: &str) -> Result<()> {
        self.c()
            .execute("DELETE FROM barcodes WHERE code = ?1", [code])?;
        Ok(())
    }

    /// Items with exactly this name (ignoring case and spaces at the ends).
    pub fn item_by_name(&self, name: &str) -> Result<Option<Item>> {
        let c = self.r();
        Ok(c.query_row(
            &format!("{ITEM_SELECT} WHERE lower(trim(i.name)) = lower(trim(?1)) GROUP BY i.id ORDER BY i.id LIMIT 1"),
            [name],
            Item::from_row,
        )
        .optional()?)
    }

    /// Items that might be the same thing: they share a word of 4+ letters
    /// with `name`. For "already in Vorrat?" after scanning an unknown code.
    pub fn similar_items(&self, name: &str, limit: usize) -> Result<Vec<Item>> {
        let words: Vec<String> = name
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.chars().count() >= 4)
            .map(|w| w.to_lowercase())
            .collect();
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let mut out: Vec<(usize, Item)> = self
            .all_items()?
            .into_iter()
            .filter_map(|it| {
                let n = it.name.to_lowercase();
                let hits = words.iter().filter(|w| n.contains(w.as_str())).count();
                (hits > 0).then_some((hits, it))
            })
            .collect();
        out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
        Ok(out.into_iter().take(limit).map(|(_, i)| i).collect())
    }

    /// Batches, soonest expiry first (FEFO order), undated last.
    pub fn batches(&self, item_id: Option<i64>) -> Result<Vec<Batch>> {
        let c = self.r();
        let order = " ORDER BY b.expires_on IS NULL, b.expires_on, b.added_on, b.id";
        let v = match item_id {
            Some(id) => c
                .prepare_cached(&format!("{BATCH_SELECT} WHERE b.item_id = ?1{order}"))?
                .query_map([id], Batch::from_row)?
                .collect::<rusqlite::Result<_>>()?,
            None => c
                .prepare_cached(&format!("{BATCH_SELECT}{order}"))?
                .query_map([], Batch::from_row)?
                .collect::<rusqlite::Result<_>>()?,
        };
        Ok(v)
    }

    pub fn add_batch(
        &self,
        item_id: i64,
        qty: f64,
        expires_on: Option<NaiveDate>,
        location_id: Option<i64>,
        price: Option<f64>,
    ) -> Result<i64> {
        if !(qty.is_finite() && qty > 0.0) {
            bail!("quantity must be positive");
        }
        let exp = expires_on.map(|d| d.to_string());
        let id = {
            let c = self.c();
            c.execute(
                "INSERT INTO batches (item_id, location_id, qty, expires_on, added_on, price)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![item_id, location_id, qty, exp, today().to_string(), price],
            )?;
            c.last_insert_rowid()
        };
        let detail = exp.as_ref().map(|e| format!("expires {e}"));
        self.log(
            Some(item_id),
            "add",
            Some(qty),
            price,
            detail.as_deref(),
            exp.as_deref(),
        )?;
        Ok(id)
    }

    pub fn delete_batch(&self, batch_id: i64) -> Result<()> {
        let row: Option<(i64, f64)> = self
            .c()
            .query_row(
                "SELECT item_id, qty FROM batches WHERE id = ?1",
                [batch_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((item_id, qty)) = row {
            self.c()
                .execute("DELETE FROM batches WHERE id = ?1", [batch_id])?;
            self.log(
                Some(item_id),
                "delete",
                Some(qty),
                None,
                Some("batch removed"),
                None,
            )?;
        }
        Ok(())
    }

    /// Use up `qty`, taking from the batch that expires first (FEFO) unless a
    /// batch is given. `waste` logs it as thrown away. Returns what was taken.
    pub fn consume(
        &self,
        item_id: i64,
        qty: f64,
        batch_id: Option<i64>,
        waste: bool,
    ) -> Result<f64> {
        if !(qty.is_finite() && qty > 0.0) {
            bail!("quantity must be positive");
        }
        let order: Vec<Batch> = self
            .batches(Some(item_id))?
            .into_iter()
            .filter(|b| batch_id.is_none_or(|id| b.id == id))
            .collect();
        let mut left = qty;
        {
            let mut c = self.c();
            let tx = c.transaction()?;
            for b in order {
                if left <= 0.0 {
                    break;
                }
                let take = b.qty.min(left);
                if take >= b.qty - 1e-9 {
                    tx.execute("DELETE FROM batches WHERE id = ?1", [b.id])?;
                } else {
                    tx.execute(
                        "UPDATE batches SET qty = ?1 WHERE id = ?2",
                        params![round6(b.qty - take), b.id],
                    )?;
                }
                left -= take;
            }
            tx.commit()?;
        }
        let taken = round6(qty - left);
        if taken > 0.0 {
            self.log(
                Some(item_id),
                if waste { "waste" } else { "use" },
                Some(taken),
                None,
                None,
                None,
            )?;
        }
        Ok(taken)
    }

    pub fn batch(&self, id: i64) -> Result<Option<(i64, Option<String>)>> {
        Ok(self
            .c()
            .query_row(
                "SELECT item_id, expires_on FROM batches WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    /// Mark a batch opened. If the item says "use within N days once opened",
    /// the date moves up to today + N (never later than the printed date), and
    /// its reminders restart. Returns the new date if it changed.
    pub fn open_batch(&self, id: i64, today: NaiveDate) -> Result<Option<NaiveDate>> {
        let Some((item_id, exp)) = self.batch(id)? else {
            return Ok(None);
        };
        self.c().execute(
            "UPDATE batches SET opened_on = ?1 WHERE id = ?2",
            params![today.to_string(), id],
        )?;
        let days: Option<i64> = self
            .c()
            .query_row(
                "SELECT open_days FROM items WHERE id = ?1",
                [item_id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        if let Some(d) = days.filter(|d| *d > 0) {
            let new = today + chrono::Days::new(d as u64);
            if exp.as_deref().is_none_or(|e| new.to_string().as_str() < e) {
                let c = self.c();
                c.execute(
                    "UPDATE batches SET expires_on = ?1 WHERE id = ?2",
                    params![new.to_string(), id],
                )?;
                c.execute("DELETE FROM notified WHERE batch_id = ?1", [id])?;
                return Ok(Some(new));
            }
        }
        Ok(None)
    }

    /// Snapshot of an item's stock before an action, so the action can be undone.
    pub fn checkpoint(&self, item_id: i64) -> Result<serde_json::Value> {
        let c = self.c();
        let mut st = c.prepare(
            "SELECT id, item_id, location_id, qty, expires_on, added_on, opened_on, price, note FROM batches WHERE item_id = ?1",
        )?;
        let batches: Vec<serde_json::Value> = st
            .query_map([item_id], |r| {
                Ok(serde_json::json!({
                    "id": r.get::<_, i64>(0)?, "item_id": r.get::<_, i64>(1)?, "location_id": r.get::<_, Option<i64>>(2)?,
                    "qty": r.get::<_, f64>(3)?, "expires_on": r.get::<_, Option<String>>(4)?, "added_on": r.get::<_, String>(5)?,
                    "opened_on": r.get::<_, Option<String>>(6)?, "price": r.get::<_, Option<f64>>(7)?, "note": r.get::<_, Option<String>>(8)?,
                }))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut st = c.prepare("SELECT n.batch_id, n.threshold, n.sent_at FROM notified n JOIN batches b ON b.id = n.batch_id WHERE b.item_id = ?1")?;
        let marks: Vec<serde_json::Value> = st
            .query_map([item_id], |r| {
                Ok(serde_json::json!({"batch_id": r.get::<_, i64>(0)?, "threshold": r.get::<_, i64>(1)?, "sent_at": r.get::<_, String>(2)?}))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let last: i64 = c.query_row("SELECT COALESCE(MAX(id), 0) FROM events", [], |r| r.get(0))?;
        Ok(
            serde_json::json!({"item_id": item_id, "batches": batches, "notified": marks, "event_after": last}),
        )
    }

    /// Store a checkpoint taken before an action that has now happened. Keeps the last 30.
    pub fn save_undo(&self, mut cp: serde_json::Value, label: &str) -> Result<String> {
        let item_id = cp["item_id"].as_i64().unwrap_or(0);
        let c = self.c();
        let to: i64 = c.query_row(
            "SELECT COALESCE(MAX(id), 0) FROM events WHERE item_id = ?1",
            [item_id],
            |r| r.get(0),
        )?;
        cp["event_to"] = to.into();
        let token = crate::files::random_hex(9);
        c.execute(
            "INSERT INTO undo (token, item_id, label, payload, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![token, item_id, label, cp.to_string(), now()],
        )?;
        c.execute("DELETE FROM undo WHERE token NOT IN (SELECT token FROM undo ORDER BY created_at DESC, rowid DESC LIMIT 30)", [])?;
        Ok(token)
    }

    /// Put the item's stock back as it was. Refuses if the item changed since.
    pub fn undo(&self, token: &str) -> Result<(Option<i64>, String)> {
        let row: Option<(String, String)> = self
            .c()
            .query_row(
                "SELECT label, payload FROM undo WHERE token = ?1",
                [token],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((label, payload)) = row else {
            return Ok((None, "Nothing to undo".into()));
        };
        let cp: serde_json::Value = serde_json::from_str(&payload)?;
        let iid = cp["item_id"].as_i64().unwrap_or(0);
        let mut c = self.c();
        let latest: i64 = c.query_row(
            "SELECT COALESCE(MAX(id), 0) FROM events WHERE item_id = ?1",
            [iid],
            |r| r.get(0),
        )?;
        let exists: bool = c.query_row(
            "SELECT EXISTS(SELECT 1 FROM items WHERE id = ?1)",
            [iid],
            |r| r.get(0),
        )?;
        if Some(latest) != cp["event_to"].as_i64() || !exists {
            return Ok((Some(iid), "Can't undo: it changed since".into()));
        }
        let tx = c.transaction()?;
        tx.execute("DELETE FROM batches WHERE item_id = ?1", [iid])?;
        for b in cp["batches"].as_array().into_iter().flatten() {
            tx.execute(
                "INSERT INTO batches (id, item_id, location_id, qty, expires_on, added_on, opened_on, price, note)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    b["id"].as_i64(), iid, b["location_id"].as_i64(), b["qty"].as_f64(), b["expires_on"].as_str(),
                    b["added_on"].as_str(), b["opened_on"].as_str(), b["price"].as_f64(), b["note"].as_str()
                ],
            )?;
        }
        for n in cp["notified"].as_array().into_iter().flatten() {
            tx.execute(
                "INSERT OR IGNORE INTO notified VALUES (?1, ?2, ?3)",
                params![
                    n["batch_id"].as_i64(),
                    n["threshold"].as_i64(),
                    n["sent_at"].as_str()
                ],
            )?;
        }
        tx.execute(
            "DELETE FROM events WHERE item_id = ?1 AND id > ?2",
            params![iid, cp["event_after"].as_i64()],
        )?;
        tx.execute("DELETE FROM undo WHERE token = ?1", [token])?;
        tx.commit()?;
        Ok((Some(iid), format!("Undone: {label}")))
    }

    pub fn add_attachment(
        &self,
        item_id: i64,
        filename: &str,
        stored: &str,
        mime: &str,
        size: i64,
    ) -> Result<i64> {
        let c = self.c();
        c.execute(
            "INSERT INTO attachments (item_id, filename, stored, mime, size, added_on) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![item_id, filename, stored, mime, size, now()],
        )?;
        Ok(c.last_insert_rowid())
    }

    pub fn attachments(&self, item_id: i64) -> Result<Vec<Attachment>> {
        let c = self.r();
        let mut st = c.prepare_cached("SELECT id, item_id, filename, stored, mime, size FROM attachments WHERE item_id = ?1 ORDER BY added_on, id")?;
        let v = st
            .query_map([item_id], Attachment::from_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(v)
    }

    pub fn attachment(&self, id: i64) -> Result<Option<Attachment>> {
        Ok(self
            .c()
            .query_row(
                "SELECT id, item_id, filename, stored, mime, size FROM attachments WHERE id = ?1",
                [id],
                Attachment::from_row,
            )
            .optional()?)
    }

    pub fn delete_attachment(&self, id: i64) -> Result<()> {
        self.c()
            .execute("DELETE FROM attachments WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Stored file names that belong to an item (attachments + picture), so
    /// deleting the item can delete the files too.
    pub fn item_files(&self, item_id: i64) -> Result<Vec<String>> {
        let mut v: Vec<String> = self
            .attachments(item_id)?
            .into_iter()
            .map(|a| a.stored)
            .collect();
        if let Some(img) = self.item(item_id)?.and_then(|i| i.image) {
            v.push(img);
        }
        Ok(v)
    }

    /// Batches with an expiry date, for the calendar feed.
    pub fn dated_batches(&self) -> Result<Vec<Batch>> {
        Ok(self
            .batches(None)?
            .into_iter()
            .filter(|b| b.expires_on.is_some())
            .collect())
    }

    pub fn targets(&self) -> Result<Vec<Target>> {
        let c = self.r();
        let mut st = c.prepare_cached(
            "SELECT id, label, category, mode, amount, unit, sort FROM targets ORDER BY sort, id",
        )?;
        let v = st
            .query_map([], |r| {
                Ok(Target {
                    id: r.get(0)?,
                    label: r.get(1)?,
                    category: r.get(2)?,
                    mode: r.get(3)?,
                    amount: r.get(4)?,
                    unit: r.get(5)?,
                    sort: r.get(6)?,
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

    pub fn shopping(&self) -> Result<Vec<ShoppingEntry>> {
        let c = self.r();
        let mut st = c.prepare_cached(
            "SELECT s.id, s.label, s.item_id, s.qty, s.done, s.added_on, i.name
             FROM shopping s LEFT JOIN items i ON i.id = s.item_id ORDER BY s.done, s.added_on, s.id",
        )?;
        let v = st
            .query_map([], |r| {
                Ok(ShoppingEntry {
                    id: r.get(0)?,
                    label: r.get(1)?,
                    item_id: r.get(2)?,
                    qty: r.get(3)?,
                    done: r.get::<_, i64>(4)? != 0,
                    added_on: r.get(5)?,
                    item_name: r.get(6)?,
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

    pub fn log(
        &self,
        item_id: Option<i64>,
        action: &str,
        qty: Option<f64>,
        price: Option<f64>,
        detail: Option<&str>,
        expires_on: Option<&str>,
    ) -> Result<()> {
        self.c().execute(
            "INSERT INTO events (ts, item_id, action, qty, price, detail, expires_on) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![now(), item_id, action, qty, price, detail, expires_on],
        )?;
        Ok(())
    }

    fn events(&self, sql: &str, args: impl rusqlite::Params) -> Result<Vec<Event>> {
        let c = self.r();
        let mut st = c.prepare_cached(sql)?;
        let v = st
            .query_map(args, |r| {
                Ok(Event {
                    id: r.get("id")?,
                    ts: r.get("ts")?,
                    item_id: r.get("item_id")?,
                    action: r.get("action")?,
                    qty: r.get("qty")?,
                    price: r.get("price")?,
                    detail: r.get("detail")?,
                    item_name: r.get("item_name")?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(v)
    }

    pub fn history(&self, item_id: i64, limit: i64) -> Result<Vec<Event>> {
        self.events(
            "SELECT e.*, NULL AS item_name FROM events e WHERE item_id = ?1 ORDER BY ts DESC, id DESC LIMIT ?2",
            params![item_id, limit],
        )
    }

    pub fn recent(&self, limit: i64) -> Result<Vec<Event>> {
        self.events(
            "SELECT e.*, i.name AS item_name FROM events e JOIN items i ON i.id = e.item_id
             WHERE e.action IN ('add', 'use', 'waste') ORDER BY e.ts DESC, e.id DESC LIMIT ?1",
            [limit],
        )
    }

    /// (timestamp, qty, expires_on) of an item's events of one kind, newest first.
    #[allow(clippy::type_complexity)]
    pub fn item_events(
        &self,
        item_id: i64,
        action: &str,
        since: &str,
        limit: i64,
    ) -> Result<Vec<(String, Option<f64>, Option<String>)>> {
        let c = self.r();
        let mut st = c.prepare_cached(
            "SELECT ts, qty, expires_on FROM events WHERE item_id = ?1 AND action = ?2 AND ts >= ?3
             ORDER BY ts DESC, id DESC LIMIT ?4",
        )?;
        let v = st
            .query_map(params![item_id, action, since, limit], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(v)
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

    /// Purchases with a price, newest first, with the price per unit.
    pub fn prices(&self, item_id: i64, limit: i64) -> Result<Vec<Price>> {
        let c = self.r();
        let mut st = c.prepare_cached(
            "SELECT ts, qty, price FROM events WHERE item_id = ?1 AND action = 'add' AND price IS NOT NULL
             ORDER BY ts DESC, id DESC LIMIT ?2",
        )?;
        let v = st
            .query_map(params![item_id, limit], |r| {
                let qty: Option<f64> = r.get(1)?;
                let price: f64 = r.get(2)?;
                Ok(Price {
                    ts: r.get(0)?,
                    qty,
                    price,
                    unit_price: qty
                        .filter(|q| *q > 0.0)
                        .map(|q| (price / q * 10_000.0).round() / 10_000.0),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(v)
    }

    /// Money spent on purchases since a date (ISO, inclusive).
    pub fn spending(&self, since: &str) -> Result<f64> {
        let v: f64 = self.r().query_row(
            "SELECT COALESCE(SUM(price), 0.0) FROM events WHERE action = 'add' AND ts >= ?1",
            [since],
            |r| r.get(0),
        )?;
        Ok((v * 100.0).round() / 100.0)
    }

    /// How many times something was thrown away since a date.
    pub fn wasted(&self, since: &str) -> Result<i64> {
        Ok(self.r().query_row(
            "SELECT COUNT(*) FROM events WHERE action = 'waste' AND ts >= ?1",
            [since],
            |r| r.get(0),
        )?)
    }

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

fn round6(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}
