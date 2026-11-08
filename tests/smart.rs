mod common;
use chrono::Days;
use common::*;
use vorrat::db::{NewItem, Store};
use vorrat::smart;

fn file_store() -> (tempfile::TempDir, Store, rusqlite::Connection) {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("v.db");
    let s = Store::open(&p).unwrap();
    let raw = rusqlite::Connection::open(&p).unwrap();
    (dir, s, raw)
}

/// Move the newest event of an item `days_ago` into the past.
fn backdate(raw: &rusqlite::Connection, item: i64, days_ago: u64) {
    let ts = (today() - Days::new(days_ago))
        .format("%Y-%m-%dT10:00:00")
        .to_string();
    raw.execute(
        "UPDATE events SET ts = ?1 WHERE id = (SELECT MAX(id) FROM events WHERE item_id = ?2)",
        rusqlite::params![ts, item],
    )
    .unwrap();
}

/// A wildly different date makes the history inconsistent: say nothing.
#[test]
fn shelf_life_needs_agreeing_purchases() {
    let (_d, s, raw) = file_store();
    let i = item(&s, "Beans");
    s.add_batch(i, 1.0, d(100), None, None).unwrap();
    assert_eq!(
        smart::shelf_life(&s, i).unwrap(),
        None,
        "one purchase is not enough"
    );
    s.add_batch(i, 1.0, d(104), None, None).unwrap();
    let sl = smart::shelf_life(&s, i).unwrap().unwrap();
    assert_eq!((sl.days, sl.n), (102, 2));
    s.add_batch(i, 1.0, d(400), None, None).unwrap();
    assert_eq!(smart::shelf_life(&s, i).unwrap(), None);
    drop(raw);
}

#[test]
fn location_guess_needs_a_clear_majority() {
    let s = store();
    let cellar = s
        .locations()
        .unwrap()
        .into_iter()
        .find(|l| l.name == "Cellar")
        .unwrap()
        .id;
    let pantry = s
        .locations()
        .unwrap()
        .into_iter()
        .find(|l| l.name == "Pantry")
        .unwrap()
        .id;
    for n in 0..3 {
        let i = item_with(
            &s,
            NewItem {
                name: format!("Water {n}"),
                category: Some("water".into()),
                ..Default::default()
            },
        );
        s.add_batch(i, 1.0, None, Some(cellar), None).unwrap();
    }
    let g = smart::guess_location(&s, Some("water"), false)
        .unwrap()
        .unwrap();
    assert_eq!(g.id, cellar);
    assert!(g.reason.contains("3 of 3"));
    let i = item_with(
        &s,
        NewItem {
            name: "Water 4".into(),
            category: Some("water".into()),
            ..Default::default()
        },
    );
    s.add_batch(i, 1.0, None, Some(pantry), None).unwrap();
    assert_eq!(
        smart::guess_location(&s, Some("water"), false).unwrap(),
        None,
        "3 of 4 is below 80 %"
    );
    assert_eq!(
        smart::guess_location(&s, None, true)
            .unwrap()
            .unwrap()
            .reason,
        "frozen product"
    );
}

#[test]
fn usual_qty_when_last_two_agree() {
    let s = store();
    let i = item(&s, "Eggs");
    s.add_batch(i, 10.0, None, None, None).unwrap();
    assert_eq!(smart::usual_qty(&s, i).unwrap(), None);
    s.add_batch(i, 10.0, None, None, None).unwrap();
    assert_eq!(smart::usual_qty(&s, i).unwrap(), Some(10.0));
    s.add_batch(i, 6.0, None, None, None).unwrap();
    assert_eq!(smart::usual_qty(&s, i).unwrap(), None);
}

/// While the item is still in stock there is no suggestion yet.
#[test]
fn buy_pattern_and_suggestion() {
    let (_d, s, raw) = file_store();
    let i = item(&s, "Milk");
    for ago in [28, 21, 14, 7] {
        s.add_batch(i, 1.0, None, None, None).unwrap();
        backdate(&raw, i, ago);
    }
    let bp = smart::buy_pattern(&s, i, today()).unwrap().unwrap();
    assert_eq!((bp.every, bp.n), (7.0, 4));
    assert!(smart::shopping_suggestions(&s, today()).unwrap().is_empty());
    s.consume(i, 4.0, None, false).unwrap();
    let sug = smart::shopping_suggestions(&s, today()).unwrap();
    assert_eq!(sug.len(), 1);
    assert!(sug[0].reason.contains("every 7 d"), "{}", sug[0].reason);
    smart::dismiss_suggestion(&s, i, today()).unwrap();
    assert!(
        smart::shopping_suggestions(&s, today()).unwrap().is_empty(),
        "dismissed for 14 days"
    );
}

#[test]
fn irregular_buying_is_no_pattern() {
    let (_d, s, raw) = file_store();
    let i = item(&s, "Chocolate");
    for ago in [60, 50, 12, 10, 2] {
        s.add_batch(i, 1.0, None, None, None).unwrap();
        backdate(&raw, i, ago);
    }
    assert_eq!(smart::buy_pattern(&s, i, today()).unwrap(), None);
}

#[test]
fn use_rate_and_insights() {
    let (_d, s, raw) = file_store();
    let i = item(&s, "Rice");
    s.add_batch(i, 10.0, None, None, None).unwrap();
    for ago in [20, 15, 10] {
        s.consume(i, 1.0, None, false).unwrap();
        backdate(&raw, i, ago);
    }
    let rate = smart::use_rate(&s, i, today()).unwrap().unwrap();
    assert!((rate - 3.0 / 20.0).abs() < 1e-9);
    let it = s.item(i).unwrap().unwrap();
    let facts = smart::insights(&s, &it, today()).unwrap();
    assert!(
        facts.iter().any(|f| f.starts_with("At your pace")),
        "{facts:?}"
    );
}
