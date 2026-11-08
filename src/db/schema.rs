//! The schema, one entry per version, and the migration loop that runs the
//! missing ones (`PRAGMA user_version` records how far a database is). The only
//! place where tables are created or changed.

use anyhow::{bail, Result};
use rusqlite::params;

use crate::db::Store;

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

/// Seeded once on a fresh DB. Per person per day, from the German civil
/// protection office (BBK) 10-day checklist divided by 10. Editable in the app.
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

impl Store {
    /// Runs the schema versions not yet applied. When settings did not exist yet
    /// as v3 ran, the household starts with two adults. Indexes are speed-ups, not
    /// a schema version, so older versions can still open the file.
    pub(crate) fn migrate(&self) -> Result<()> {
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
}
