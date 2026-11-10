//! Queries for items, batches, barcodes, attachments, undo and the event history.

use anyhow::{bail, Result};
use chrono::NaiveDate;
use rusqlite::{params, OptionalExtension, Row};
use serde::Serialize;

use crate::db::{now, today, Store};
use crate::stock::rules::Snapshot;

pub const KINDS: &[&str] = &["food", "supply", "document", "thing"];

pub const UNITS: &[&str] = &[
    "pcs", "g", "kg", "ml", "L", "EUR", "CHF", "CZK", "DKK", "NOK", "PLN", "SEK",
];

/// The units in [`UNITS`] that are money: the currencies of the countries
/// with a verified guide. Cash is counted in its own currency.
pub const CURRENCIES: &[&str] = &["EUR", "CHF", "CZK", "DKK", "NOK", "PLN", "SEK"];

/// Categories double as the link between stock and the emergency plan (targets).
pub const CATEGORIES: &[&str] = &[
    "water",
    "washing water",
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
            id: r.get("id")?,
            item_id: r.get("item_id")?,
            filename: r.get("filename")?,
            stored: r.get("stored")?,
            mime: r.get("mime")?,
            size: r.get("size")?,
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

fn round6(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

impl Store {
    /// Every item and batch, read once for a page.
    pub fn snapshot(&self, today: NaiveDate) -> Result<Snapshot> {
        Ok(Snapshot::new(self.all_items()?, self.batches(None)?, today))
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
                    "id": r.get::<_, i64>("id")?,
                    "item_id": r.get::<_, i64>("item_id")?,
                    "location_id": r.get::<_, Option<i64>>("location_id")?,
                    "qty": r.get::<_, f64>("qty")?,
                    "expires_on": r.get::<_, Option<String>>("expires_on")?,
                    "added_on": r.get::<_, String>("added_on")?,
                    "opened_on": r.get::<_, Option<String>>("opened_on")?,
                    "price": r.get::<_, Option<f64>>("price")?,
                    "note": r.get::<_, Option<String>>("note")?,
                }))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut st = c.prepare("SELECT n.batch_id, n.threshold, n.sent_at FROM notified n JOIN batches b ON b.id = n.batch_id WHERE b.item_id = ?1")?;
        let marks: Vec<serde_json::Value> = st
            .query_map([item_id], |r| {
                Ok(serde_json::json!({
                    "batch_id": r.get::<_, i64>("batch_id")?,
                    "threshold": r.get::<_, i64>("threshold")?,
                    "sent_at": r.get::<_, String>("sent_at")?,
                }))
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

    /// Purchases with a price, newest first, with the price per unit.
    pub fn prices(&self, item_id: i64, limit: i64) -> Result<Vec<Price>> {
        let c = self.r();
        let mut st = c.prepare_cached(
            "SELECT ts, qty, price FROM events WHERE item_id = ?1 AND action = 'add' AND price IS NOT NULL
             ORDER BY ts DESC, id DESC LIMIT ?2",
        )?;
        let v = st
            .query_map(params![item_id, limit], |r| {
                let qty: Option<f64> = r.get("qty")?;
                let price: f64 = r.get("price")?;
                Ok(Price {
                    ts: r.get("ts")?,
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
}
