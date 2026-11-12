//! Template filters and small helpers that format values for pages.

use chrono::NaiveDate;

use crate::i18n::{tr, trf};

tokio::task_local! {
    /// Whether this request is on a demo instance.
    pub(crate) static DEMO: bool;
}

/// Template filters: `{{ "Stock"|t }}`, and the demo banner text.
pub(crate) mod filters {
    pub fn t<T: std::fmt::Display>(s: T, _: &dyn askama::Values) -> askama::Result<String> {
        Ok(crate::i18n::tr(&s.to_string()))
    }

    pub fn lang_code<T: std::fmt::Display>(
        _: T,
        _: &dyn askama::Values,
    ) -> askama::Result<&'static str> {
        Ok(crate::i18n::current().code())
    }

    pub fn demo_note<T: std::fmt::Display>(_: T, _: &dyn askama::Values) -> askama::Result<String> {
        let on = super::DEMO.try_with(|d| *d).unwrap_or(false);
        Ok(if on {
            crate::i18n::tr("Demo: try anything. The data resets every night.")
        } else {
            String::new()
        })
    }
}

/// "€12.50", "CHF 3.20".
pub fn money(v: f64, currency: &str) -> String {
    let sym = match currency {
        "EUR" => "€",
        "USD" => "$",
        "GBP" => "£",
        _ => "",
    };
    if sym.is_empty() {
        format!("{currency} {v:.2}")
    } else {
        format!("{sym}{v:.2}")
    }
}

pub(crate) fn pct(r: f64) -> u32 {
    (r.clamp(0.0, 1.0) * 100.0).round() as u32
}

pub(crate) fn bar_class(r: f64) -> &'static str {
    if r < 0.4 {
        "crit"
    } else if r < 0.9 {
        "warn"
    } else {
        ""
    }
}

/// One square per planned day, filled for each whole day the stock lasts:
/// the day squares on Home and Plan. Empty when there is no day count, or
/// when more than 31 days are planned and a bar reads better.
pub(crate) fn day_cells(lasts: Option<f64>, planned: i64) -> Vec<bool> {
    match lasts {
        Some(d) if (1..=31).contains(&planned) => {
            let full = d.clamp(0.0, planned as f64).floor() as i64;
            (0..planned).map(|i| i < full).collect()
        }
        _ => Vec::new(),
    }
}

pub(crate) fn when(d: Option<i64>) -> String {
    match d {
        None => tr("no date"),
        Some(0) => tr("today"),
        Some(1) => tr("tomorrow"),
        Some(d) if d < 0 => trf("{0}d ago", &[&-d]),
        Some(d) if d < 60 => trf("{0}d", &[&d]),
        Some(d) => trf("{0}mo", &[&(d / 30)]),
    }
}

pub(crate) fn hdate(s: Option<&str>) -> String {
    let fmt = tr("%-d %b %Y");
    s.and_then(|v| NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
        .map(|d| d.format(&fmt).to_string())
        .unwrap_or_else(|| tr("no date"))
}
