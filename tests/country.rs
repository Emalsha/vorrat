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

/// Cash amounts are those the verified guides give, per adult or per child.
#[test]
fn cash_amounts_match_the_guidance_file() {
    let text = std::fs::read_to_string("docs/guidance/presets.json").unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    for s in json["sources"].as_array().unwrap() {
        let code = s["code"].as_str().unwrap();
        let mut want: Vec<(String, f64, String)> = s["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["category"] == "cash" && i["quantity"].is_number())
            .filter(|_| s["status"] == "verified")
            .map(|i| {
                (
                    i["basis"].as_str().unwrap().to_string(),
                    i["quantity"].as_f64().unwrap(),
                    i["unit"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        let mut have: Vec<(String, f64, String)> = find(code)
            .unwrap()
            .cash
            .iter()
            .map(|(m, a, u)| (m.to_string(), *a, u.to_string()))
            .collect();
        want.sort_by(|a, b| a.0.cmp(&b.0));
        have.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(have, want, "{code}");
    }
}

/// Cash counts per adult and per child, and only in the target's currency.
#[test]
fn cash_targets_count_adults_children_and_one_currency() {
    use vorrat::stock::queries::NewItem;
    let s = common::store();
    s.add_member("person", "child", "", &[]).unwrap();
    s.add_member("person", "teen", "", &[]).unwrap();
    s.add_target("Cash in small notes", "cash", "per_adult", 70.0, "EUR")
        .unwrap();
    s.add_target("Cash in small notes", "cash", "per_child", 30.0, "EUR")
        .unwrap();
    let cash = |unit: &str| {
        common::item_with(
            &s,
            NewItem {
                name: format!("Cash {unit}"),
                category: Some("cash".into()),
                unit: Some(unit.into()),
                ..Default::default()
            },
        )
    };
    s.add_batch(cash("EUR"), 100.0, None, None, None).unwrap();
    s.add_batch(cash("SEK"), 500.0, None, None, None).unwrap();
    let plan = s.plan(common::today()).unwrap();
    let lines: Vec<_> = plan
        .lines
        .iter()
        .filter(|c| c.target.category == "cash")
        .collect();
    assert_eq!(lines[0].need, 140.0, "two adults");
    assert_eq!(lines[1].need, 60.0, "a child and a teen");
    assert_eq!(lines[0].have, 100.0, "kronor don't count as euro");
    assert!(lines[0].days.is_none());

    let nl = find("NL").unwrap();
    assert!(
        changes(nl, 3, &s.targets().unwrap()).cash.is_empty(),
        "already there"
    );
}

/// Iodine notes only for verified guides that mention iodine, and translated.
#[test]
fn iodine_notes_come_from_verified_guides() {
    let text = std::fs::read_to_string("docs/guidance/presets.json").unwrap();
    for (code, note) in vorrat::country::rules::IODINE {
        assert_eq!(find(code).unwrap().status, Status::Verified, "{code}");
        assert!(Lang::De.table().contains_key(note), "{note}");
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        let source = json["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["code"] == *code)
            .unwrap();
        assert!(
            source.to_string().to_lowercase().contains("iodine"),
            "{code} says nothing about iodine"
        );
    }
}
