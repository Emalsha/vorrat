//! Translations. Templates mark text with `{{ "Stock"|t }}`; the English text
//! is the key, and `locales/<code>.tsv` holds one `English<TAB>translation`
//! line per text. Anything without a translation stays English. Sentences with
//! numbers use `{0}`, `{1}` placeholders (see [`trf`]).

use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    De,
    Fr,
    Nl,
}

/// (language, code, name in that language, translation file)
pub const LANGS: &[(Lang, &str, &str, &str)] = &[
    (Lang::En, "en", "English", ""),
    (Lang::De, "de", "Deutsch", include_str!("../locales/de.tsv")),
    (
        Lang::Fr,
        "fr",
        "Français",
        include_str!("../locales/fr.tsv"),
    ),
    (
        Lang::Nl,
        "nl",
        "Nederlands",
        include_str!("../locales/nl.tsv"),
    ),
];

impl Lang {
    pub fn code(self) -> &'static str {
        LANGS.iter().find(|l| l.0 == self).map_or("en", |l| l.1)
    }

    fn from_code(code: &str) -> Option<Lang> {
        LANGS.iter().find(|l| l.1 == code).map(|l| l.0)
    }

    /// The saved choice (a language code), or else the browser's first
    /// supported language, or English.
    pub fn pick(setting: &str, accept_language: Option<&str>) -> Lang {
        if let Some(l) = Lang::from_code(setting) {
            return l;
        }
        for part in accept_language.unwrap_or("").split(',') {
            let tag = part
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            if let Some(l) = Lang::from_code(tag.get(..2).unwrap_or("")) {
                return l;
            }
        }
        Lang::En
    }

    /// This language's table (empty for English).
    pub fn table(self) -> &'static HashMap<&'static str, &'static str> {
        static TABLES: OnceLock<Vec<(Lang, HashMap<&'static str, &'static str>)>> = OnceLock::new();
        let all = TABLES.get_or_init(|| LANGS.iter().map(|l| (l.0, parse(l.3))).collect());
        &all.iter()
            .find(|t| t.0 == self)
            .expect("every language has a table")
            .1
    }
}

/// `English<TAB>translation` lines; `#` starts a comment line.
pub fn parse(tsv: &'static str) -> HashMap<&'static str, &'static str> {
    tsv.lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split_once('\t'))
        .collect()
}

tokio::task_local! {
    /// The language of the request being handled.
    pub static LANG: Lang;
}

pub fn current() -> Lang {
    LANG.try_with(|l| *l).unwrap_or(Lang::En)
}

/// Translate into the current request's language.
pub fn tr(s: &str) -> String {
    current()
        .table()
        .get(s)
        .map_or_else(|| s.to_string(), |t| t.to_string())
}

/// Translate a sentence with `{0}`, `{1}`, … placeholders, then fill them in.
pub fn trf(s: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = tr(s);
    for (i, a) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), &a.to_string());
    }
    out
}
