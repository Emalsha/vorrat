//! Getting data out and in: a full JSON export, the stock as CSV (opens in
//! any spreadsheet), and a CSV import that adds stock.

use std::collections::HashMap;

use anyhow::{bail, Result};
use chrono::NaiveDate;

use crate::db::{NewItem, Store, EXPORT_TABLES};

/// Everything the household entered, one JSON document. Attachments are
/// listed by file name; the files themselves are in the backup archive.
/// Medicines and health needs are the most private part, so the file says so
/// when it holds them.
pub fn export_json(store: &Store, now: &str) -> Result<serde_json::Value> {
    let mut tables = serde_json::Map::new();
    for t in EXPORT_TABLES {
        let rows = store.dump_table(t)?;
        tables.insert(
            t.to_string(),
            serde_json::Value::Array(rows.into_iter().map(Into::into).collect()),
        );
    }
    let health = ["doses", "members"].iter().any(|t| {
        tables[*t].as_array().is_some_and(|rows| {
            rows.iter()
                .any(|r| *t == "doses" || r["needs"].as_str().is_some_and(|n| !n.is_empty()))
        })
    });
    let mut doc = serde_json::json!({
        "format": "vorrat-export",
        "schema": store.schema_version()?,
        "version": env!("CARGO_PKG_VERSION"),
        "exported_at": now,
        "tables": tables,
    });
    if health {
        doc["private"] = "Contains medicines and health needs. Keep this file private.".into();
    }
    Ok(doc)
}

pub const CSV_COLUMNS: &[&str] = &[
    "name",
    "brand",
    "kind",
    "category",
    "barcode",
    "qty",
    "unit",
    "expires_on",
    "location",
    "price",
    "min_qty",
    "added_on",
];

/// One row per batch in stock, plus items with no stock (qty 0), so the file
/// round-trips through [`import_csv`].
pub fn export_csv(store: &Store) -> Result<String> {
    let mut out = String::new();
    push_row(&mut out, CSV_COLUMNS.iter().map(|s| s.to_string()));
    let batches = store.batches(None)?;
    let items = store.all_items()?;
    let by_id: HashMap<i64, &crate::db::Item> = items.iter().map(|i| (i.id, i)).collect();
    let num = |v: f64| crate::rules::fmt_qty(v);
    let mut stocked = std::collections::HashSet::new();
    for b in &batches {
        let Some(it) = by_id.get(&b.item_id) else {
            continue;
        };
        stocked.insert(it.id);
        push_row(
            &mut out,
            [
                it.name.clone(),
                it.brand.clone().unwrap_or_default(),
                it.kind.clone(),
                it.category.clone().unwrap_or_default(),
                it.barcode.clone().unwrap_or_default(),
                num(b.qty),
                it.unit.clone(),
                b.expires_on.clone().unwrap_or_default(),
                b.location_name.clone().unwrap_or_default(),
                b.price.map(num).unwrap_or_default(),
                num(it.min_qty),
                b.added_on.clone(),
            ],
        );
    }
    for it in items.iter().filter(|i| !stocked.contains(&i.id)) {
        push_row(
            &mut out,
            [
                it.name.clone(),
                it.brand.clone().unwrap_or_default(),
                it.kind.clone(),
                it.category.clone().unwrap_or_default(),
                it.barcode.clone().unwrap_or_default(),
                "0".into(),
                it.unit.clone(),
                String::new(),
                String::new(),
                String::new(),
                num(it.min_qty),
                String::new(),
            ],
        );
    }
    Ok(out)
}

/// One CSV line. Spreadsheets run cells starting with = + - @ as formulas, so
/// such cells get a leading quote to keep them text (CSV injection).
fn push_row(out: &mut String, cells: impl IntoIterator<Item = String>) {
    let mut first = true;
    for c in cells {
        if !first {
            out.push(',');
        }
        first = false;
        let c = if c.starts_with(['=', '+', '-', '@']) && c.parse::<f64>().is_err() {
            format!("'{c}")
        } else {
            c
        };
        if c.contains([',', '"', '\n', '\r', ';']) {
            out.push('"');
            out.push_str(&c.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(&c);
        }
    }
    out.push_str("\r\n");
}

/// Parse CSV with `,`, `;` or tab as separator (whichever the header uses most;
/// European spreadsheets save with `;`). Handles quotes and newlines in quotes.
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let text = text.trim_start_matches('\u{feff}');
    let header = text.lines().next().unwrap_or("");
    let sep = [',', ';', '\t']
        .into_iter()
        .max_by_key(|c| header.matches(*c).count())
        .unwrap_or(',');
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if quoted {
            match ch {
                '"' if chars.peek() == Some(&'"') => {
                    cell.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                _ => cell.push(ch),
            }
        } else if ch == '"' && cell.is_empty() {
            quoted = true;
        } else if ch == sep {
            row.push(std::mem::take(&mut cell));
        } else if ch == '\n' || ch == '\r' {
            if ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut cell));
            if row.iter().any(|c| !c.trim().is_empty()) {
                rows.push(std::mem::take(&mut row));
            } else {
                row.clear();
            }
        } else {
            cell.push(ch);
        }
    }
    row.push(cell);
    if row.iter().any(|c| !c.trim().is_empty()) {
        rows.push(row);
    }
    rows
}

/// What a CSV import did.
///
/// - `skipped`: (line number, why)
#[derive(Debug, Default, PartialEq)]
pub struct ImportReport {
    pub rows: usize,
    pub new_items: usize,
    pub batches: usize,
    pub skipped: Vec<(usize, String)>,
}

impl ImportReport {
    pub fn summary(&self) -> String {
        let mut s = format!(
            "Imported {} rows: {} new items, {} stock entries",
            self.rows, self.new_items, self.batches
        );
        if !self.skipped.is_empty() {
            let lines: Vec<String> = self
                .skipped
                .iter()
                .take(5)
                .map(|(l, w)| format!("line {l}: {w}"))
                .collect();
            s.push_str(&format!(
                "; skipped {} ({})",
                self.skipped.len(),
                lines.join("; ")
            ));
        }
        s
    }
}

fn num(s: &str) -> Option<f64> {
    let s = s.trim().trim_start_matches('\'');
    if s.is_empty() {
        return None;
    }
    s.replace(',', ".")
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite())
}

/// `2027-05-01`, `01.05.2027` or `01/05/2027` (day first, as in Europe).
fn date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    ["%Y-%m-%d", "%d.%m.%Y", "%d/%m/%Y", "%d.%m.%y"]
        .iter()
        .find_map(|f| NaiveDate::parse_from_str(s, f).ok())
}

const MAX_ROWS: usize = 10_000;

/// Add the rows to the stock. Matches an existing item by barcode, then by
/// name; creates it otherwise. Never deletes or changes existing stock.
pub fn import_csv(store: &Store, text: &str) -> Result<ImportReport> {
    let rows = parse_csv(text);
    let Some((head, body)) = rows.split_first() else {
        bail!("the file is empty")
    };
    let col: HashMap<String, usize> = head
        .iter()
        .enumerate()
        .map(|(i, h)| (h.trim().to_lowercase(), i))
        .collect();
    if !col.contains_key("name") {
        bail!(
            "the first line needs a \"name\" column (columns: {})",
            CSV_COLUMNS.join(", ")
        );
    }
    if body.len() > MAX_ROWS {
        bail!("at most {MAX_ROWS} rows at once");
    }
    let mut locs: HashMap<String, i64> = store
        .locations()?
        .into_iter()
        .map(|l| (l.name.trim().to_lowercase(), l.id))
        .collect();
    let mut rep = ImportReport::default();
    for (n, r) in body.iter().enumerate() {
        let line = n + 2;
        let get = |k: &str| {
            col.get(k)
                .and_then(|i| r.get(*i))
                .map(|s| s.trim().trim_start_matches('\'').to_string())
                .unwrap_or_default()
        };
        let opt = |k: &str| Some(get(k)).filter(|s| !s.is_empty());
        let name = get("name");
        if name.is_empty() {
            rep.skipped.push((line, "no name".into()));
            continue;
        }
        let qty = match get("qty").as_str() {
            "" => 0.0,
            q => match num(q) {
                Some(v) if v >= 0.0 => v,
                _ => {
                    rep.skipped.push((line, format!("quantity {q:?}")));
                    continue;
                }
            },
        };
        let expires = match get("expires_on").as_str() {
            "" => None,
            d => match date(d) {
                Some(v) => Some(v),
                None => {
                    rep.skipped.push((line, format!("date {d:?}")));
                    continue;
                }
            },
        };
        let barcode = opt("barcode");
        let existing = match &barcode {
            Some(b) => store.item_by_barcode(b)?,
            None => None,
        };
        let existing = match existing {
            Some(i) => Some(i),
            None => store.item_by_name(&name)?,
        };
        let item_id = match existing {
            Some(i) => i.id,
            None => {
                let kind = opt("kind").map(|k| k.to_lowercase());
                let unit = opt("unit");
                match store.add_item(NewItem {
                    name: name.clone(),
                    kind,
                    barcode: barcode.clone(),
                    brand: opt("brand"),
                    category: opt("category").map(|c| c.to_lowercase()),
                    unit,
                    min_qty: num(&get("min_qty")),
                    ..Default::default()
                }) {
                    Ok(id) => {
                        rep.new_items += 1;
                        id
                    }
                    Err(e) => {
                        rep.skipped.push((line, e.to_string()));
                        continue;
                    }
                }
            }
        };
        if qty > 0.0 {
            let loc = match opt("location") {
                Some(l) => {
                    let key = l.to_lowercase();
                    match locs.get(&key) {
                        Some(id) => Some(*id),
                        None => {
                            let id = store.add_location(&l, None)?;
                            locs.insert(key, id);
                            Some(id)
                        }
                    }
                }
                None => None,
            };
            store.add_batch(item_id, qty, expires, loc, num(&get("price")))?;
            rep.batches += 1;
        }
        rep.rows += 1;
    }
    Ok(rep)
}
