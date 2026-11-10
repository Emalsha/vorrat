//! Country presets: the table in `src/country/rules.rs` matches
//! `docs/guidance/presets.json`, only verified guides carry numbers, and
//! using a country changes only what differs.

mod common;

use vorrat::country::rules::{changes, find, water_target, Status, COUNTRIES};
use vorrat::i18n::Lang;

/// Every authority in `presets.json` is in the table with the same link,
/// status, days and water; numbers only for verified guides.
#[test]
fn the_table_matches_the_guidance_file() {
    let text = std::fs::read_to_string("docs/guidance/presets.json").unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let sources = json["sources"].as_array().unwrap();
    assert_eq!(sources.len(), COUNTRIES.len());
    for s in sources {
        let code = s["code"].as_str().unwrap();
        let c = find(code).unwrap_or_else(|| panic!("{code} is missing"));
        assert_eq!(c.url, s["url"].as_str().unwrap(), "{code}");
        let verified = s["status"] == "verified";
        let status = match s["status"].as_str().unwrap() {
            "verified" => Status::Verified,
            "partial" => Status::Partial,
            _ => Status::Unverified,
        };
        assert_eq!(c.status, status, "{code}");
        let days = s["days"].as_i64().filter(|_| verified);
        assert_eq!(c.days, days, "{code} days");
        let water = s["water_l_per_person_day"].as_f64().filter(|_| verified);
        assert_eq!(c.water, water, "{code} water");
    }
}

#[test]
fn every_country_name_is_translated() {
    for c in COUNTRIES {
        assert!(Lang::De.table().contains_key(c.name), "{}", c.name);
    }
}

#[test]
fn only_what_differs_changes() {
    let s = common::store();
    let targets = s.targets().unwrap();
    let de = find("DE").unwrap();
    let ch = changes(de, 10, &targets);
    assert!(ch.is_empty(), "the defaults are the BBK's: {ch:?}");
    let ch = changes(de, 14, &targets);
    assert_eq!(ch.days, Some((14, 10)));
    assert!(ch.water.is_none());

    let se = find("SE").unwrap();
    let ch = changes(se, 7, &targets);
    let w = water_target(&targets).unwrap();
    assert_eq!(ch.water, Some((Some(w.id), Some(2.0), 3.0)));
    assert!(ch.days.is_none());

    s.delete_target(w.id).unwrap();
    let ch = changes(se, 7, &s.targets().unwrap());
    assert_eq!(
        ch.water,
        Some((None, None, 3.0)),
        "a new target when none is left"
    );

    assert!(changes(find("IT").unwrap(), 14, &targets).is_empty());
}
