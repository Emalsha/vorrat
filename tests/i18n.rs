use std::collections::HashMap;
use vorrat::i18n::{parse, Lang, LANGS};

fn de() -> &'static HashMap<&'static str, &'static str> {
    Lang::De.table()
}

/// Every template under `templates/`, with its path and text.
fn templates() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut dirs = vec![std::path::PathBuf::from("templates")];
    while let Some(d) = dirs.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                dirs.push(p);
            } else {
                let text = std::fs::read_to_string(&p).unwrap();
                out.push((p.display().to_string(), text));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn every_marked_text_has_a_german_translation() {
    let re = regex::Regex::new(r#""((?:[^"\\]|\\.)*)"\|t"#).unwrap();
    let mut missing = Vec::new();
    for (p, text) in templates() {
        for c in re.captures_iter(&text) {
            let k = c[1].replace("\\\"", "\"");
            if !de().contains_key(k.as_str()) {
                missing.push(format!("{p}: {k}"));
            }
        }
    }
    let checks = vorrat::essentials::rules::FIRST_AID
        .iter()
        .chain(vorrat::essentials::rules::ESSENTIALS)
        .chain(vorrat::essentials::rules::SUGGESTIONS)
        .map(|c| &c.label);
    for k in vorrat::stock::queries::KINDS
        .iter()
        .chain(vorrat::stock::queries::CATEGORIES)
        .chain(vorrat::essentials::queries::MED_FORMS)
        .chain(checks)
    {
        if !de().contains_key(k) {
            missing.push(format!("db: {k}"));
        }
    }
    for (label, ..) in vorrat::db::schema::DEFAULT_TARGETS {
        if !de().contains_key(label) {
            missing.push(format!("target: {label}"));
        }
    }
    assert!(
        missing.is_empty(),
        "missing German:\n{}",
        missing.join("\n")
    );
}

/// German is the reference: every other language has the same keys, keeps
/// the placeholders, and has no stale keys.
#[test]
fn every_language_is_complete() {
    let ph = regex::Regex::new(r"\{\d\}").unwrap();
    let places = |s: &str| {
        let mut v: Vec<String> = ph.find_iter(s).map(|m| m.as_str().to_string()).collect();
        v.sort();
        v
    };
    for (lang, code, _, tsv) in LANGS.iter().filter(|l| l.1 != "en") {
        let t = parse(tsv);
        assert_eq!(
            t.len(),
            tsv.lines()
                .filter(|l| !l.starts_with('#') && !l.is_empty())
                .count(),
            "{code}: a key is listed twice or a line has no tab"
        );
        let missing: Vec<_> = de().keys().filter(|k| !t.contains_key(*k)).collect();
        assert!(missing.is_empty(), "{code} is missing: {missing:#?}");
        let stale: Vec<_> = t.keys().filter(|k| !de().contains_key(*k)).collect();
        assert!(stale.is_empty(), "{code} has unknown keys: {stale:#?}");
        for (k, v) in &t {
            assert_eq!(places(k), places(v), "{code}: placeholders in {k:?}");
            assert!(!v.trim().is_empty(), "{code}: empty translation for {k:?}");
        }
        assert_eq!(lang.code(), *code);
    }
}

#[test]
fn language_choice() {
    assert_eq!(
        Lang::pick("auto", Some("de-DE,de;q=0.9,en;q=0.8")),
        Lang::De
    );
    assert_eq!(
        Lang::pick("auto", Some("fr-FR,en;q=0.8,de;q=0.5")),
        Lang::Fr
    );
    assert_eq!(Lang::pick("auto", Some("nl-BE")), Lang::Nl);
    assert_eq!(Lang::pick("auto", Some("pt-PT,de;q=0.5")), Lang::De);
    assert_eq!(Lang::pick("auto", Some("de-AT")), Lang::De);
    assert_eq!(Lang::pick("en", Some("de")), Lang::En);
    assert_eq!(Lang::pick("de", None), Lang::De);
    assert_eq!(Lang::pick("", None), Lang::En);
}
