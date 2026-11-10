//! The folder layout from `docs/ARCHITECTURE.md`: rules files stay free of
//! the web and the database, shared modules name no feature, and every
//! feature's routes are merged into the router.

use std::fs;

/// The feature folders: each has pages and its own routes.
const FEATURES: &[&str] = &[
    "home",
    "stock",
    "locations",
    "plan",
    "household",
    "country",
    "grab_bag",
    "essentials",
    "shopping",
    "settings",
    "api",
];

/// Shared modules: every feature may use them, they use no feature.
const SHARED: &[&str] = &[
    "src/db/mod.rs",
    "src/db/schema.rs",
    "src/web/respond.rs",
    "src/web/form.rs",
    "src/web/view.rs",
    "src/auth/mod.rs",
    "src/auth/pages.rs",
    "src/config.rs",
    "src/i18n.rs",
    "src/files.rs",
];

/// The names of the features a source text uses, as `crate::x` or inside
/// `crate::{...}`.
fn features_used(text: &str) -> Vec<&'static str> {
    let grouped = regex::Regex::new(r"crate::\{([^}]*)\}").unwrap();
    let mut names: Vec<String> = grouped
        .captures_iter(text)
        .flat_map(|c| {
            c[1].split(',')
                .map(|n| n.trim().split("::").next().unwrap_or("").to_string())
                .collect::<Vec<_>>()
        })
        .collect();
    let single = regex::Regex::new(r"crate::(\w+)").unwrap();
    names.extend(single.captures_iter(text).map(|c| c[1].to_string()));
    FEATURES
        .iter()
        .copied()
        .filter(|f| names.iter().any(|n| n == f))
        .collect()
}

#[test]
fn rules_files_use_no_web_and_no_database() {
    let mut problems = Vec::new();
    for e in fs::read_dir("src").unwrap() {
        let rules = e.unwrap().path().join("rules.rs");
        if !rules.exists() {
            continue;
        }
        let text = fs::read_to_string(&rules).unwrap();
        for word in ["axum", "askama", "rusqlite", "crate::db", "Store"] {
            if text.contains(word) {
                problems.push(format!("{} uses {word}", rules.display()));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn shared_modules_name_no_feature() {
    let mut problems = Vec::new();
    for path in SHARED {
        let text = fs::read_to_string(path).unwrap();
        for f in features_used(&text) {
            problems.push(format!("{path} uses the feature {f}"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn every_feature_is_merged_into_the_router() {
    let router = fs::read_to_string("src/web/mod.rs").unwrap();
    for f in FEATURES {
        let module = format!("src/{f}/mod.rs");
        let text = fs::read_to_string(&module).unwrap();
        assert!(text.contains("pub fn routes()"), "{module} has no routes()");
        assert!(
            router.contains(&format!("{f}::routes()")),
            "src/web/mod.rs doesn't merge {f}::routes()"
        );
    }
    for e in fs::read_dir("src").unwrap() {
        let dir = e.unwrap().path();
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        if dir.join("pages.rs").exists() && name != "auth" {
            assert!(
                FEATURES.contains(&name.as_str()),
                "src/{name} has pages but is not in FEATURES"
            );
        }
    }
}

#[test]
fn the_checker_sees_what_it_should() {
    assert_eq!(
        features_used("use crate::stock::rules::Snapshot;"),
        ["stock"]
    );
    assert_eq!(
        features_used("use crate::{db, plan, stock};"),
        ["stock", "plan"]
    );
    assert!(features_used("use crate::db::Store;").is_empty());
}
