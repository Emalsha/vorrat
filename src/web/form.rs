//! Reading optional values from form fields.

use chrono::NaiveDate;

/// An optional number from a form field. A decimal comma is accepted: half of
/// Europe types "1,5".
pub(crate) fn opt_f64(s: &Option<String>) -> anyhow::Result<Option<f64>> {
    match s.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(v) => v
            .replace(',', ".")
            .parse::<f64>()
            .map(Some)
            .map_err(|_| anyhow::anyhow!("{v:?} is not a number")),
    }
}

pub(crate) fn opt_i64(s: &Option<String>) -> Option<i64> {
    s.as_deref().and_then(|v| v.trim().parse().ok())
}

pub(crate) fn opt_date(s: &Option<String>) -> anyhow::Result<Option<NaiveDate>> {
    match s.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(v) => NaiveDate::parse_from_str(v, "%Y-%m-%d")
            .map(Some)
            .map_err(|_| anyhow::anyhow!("{v:?} is not a date")),
    }
}
