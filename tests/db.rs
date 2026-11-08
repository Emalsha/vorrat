mod common;
use common::*;
use vorrat::db::{ItemQuery, NewItem, Store};

#[test]
fn fresh_db_is_seeded() {
    let s = store();
    assert_eq!(s.schema_version().unwrap(), 3);
    assert_eq!(s.targets().unwrap().len(), 10);
    assert_eq!(s.locations().unwrap().len(), 7);
    assert_eq!(s.setting_int("people", 0), 2);
    assert_eq!(
        s.household_counts().unwrap(),
        (2, 0),
        "starts with two adults"
    );
    assert_eq!(s.setting_int("days", 0), 14);
}

#[test]
fn migrate_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("v.db");
    {
        let s = Store::open(&p).unwrap();
        item(&s, "Rice");
    }
    let s = Store::open(&p).unwrap();
    assert_eq!(s.all_items().unwrap().len(), 1);
    assert_eq!(
        s.targets().unwrap().len(),
        10,
        "seed data must not be duplicated"
    );
}

#[test]
fn opens_a_schema_v2_database() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("vorrat.db");
    std::fs::copy("tests/fixtures/schema-v2.db", &p).unwrap();
    let s = Store::open(&p).unwrap();
    // Upgraded to v3: the "people" setting (2) became two adults.
    assert_eq!(s.schema_version().unwrap(), 3);
    let m = s.members().unwrap();
    assert_eq!(m.len(), 2);
    assert!(m.iter().all(|m| m.kind == "person" && m.r#type == "adult"));
    let water = s.item_by_barcode("4001234567890").unwrap().unwrap();
    assert_eq!(water.total, 10.0);
    let rice = s
        .item_by_barcode("4009999999999")
        .unwrap()
        .expect("linked barcode");
    assert_eq!(rice.name, "Rice");
    assert!(rice.is_low());
    assert!(s
        .history(water.id, 10)
        .unwrap()
        .iter()
        .any(|e| e.action == "use"));
}

#[test]
fn refuses_a_newer_schema() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("v.db");
    {
        let c = rusqlite::Connection::open(&p).unwrap();
        c.pragma_update(None, "user_version", 99).unwrap();
    }
    assert!(Store::open(&p).is_err());
}

#[test]
fn location_paths() {
    let s = store();
    let k = s.add_location("House", None).unwrap();
    let p = s.add_location("Pantry", Some(k)).unwrap();
    s.add_location("Shelf 2", Some(p)).unwrap();
    let paths: Vec<String> = s.locations().unwrap().into_iter().map(|l| l.path).collect();
    assert!(paths.contains(&"House › Pantry › Shelf 2".to_string()));
}

#[test]
fn consume_takes_earliest_expiry_first() {
    let s = store();
    let i = item(&s, "Milk");
    let late = s.add_batch(i, 2.0, d(10), None, None).unwrap();
    let early = s.add_batch(i, 1.0, d(2), None, None).unwrap();
    s.add_batch(i, 5.0, None, None, None).unwrap(); // undated goes last
    assert_eq!(s.consume(i, 2.0, None, false).unwrap(), 2.0);
    let left: Vec<(i64, f64)> = s
        .batches(Some(i))
        .unwrap()
        .iter()
        .map(|b| (b.id, b.qty))
        .collect();
    assert!(!left.iter().any(|(id, _)| *id == early));
    assert_eq!(left[0], (late, 1.0));
}

#[test]
fn consume_more_than_stock_takes_what_is_there() {
    let s = store();
    let i = item(&s, "Tea");
    s.add_batch(i, 1.5, None, None, None).unwrap();
    assert_eq!(s.consume(i, 4.0, None, false).unwrap(), 1.5);
    assert!(s.batches(Some(i)).unwrap().is_empty());
}

#[test]
fn consume_specific_batch() {
    let s = store();
    let i = item(&s, "Beans");
    s.add_batch(i, 1.0, d(1), None, None).unwrap();
    let b = s.add_batch(i, 3.0, d(30), None, None).unwrap();
    s.consume(i, 1.0, Some(b), false).unwrap();
    let qtys: Vec<f64> = s.batches(Some(i)).unwrap().iter().map(|b| b.qty).collect();
    assert_eq!(qtys, vec![1.0, 2.0]);
}

#[test]
fn bad_input_is_rejected() {
    let s = store();
    let i = item(&s, "X");
    assert!(s.add_batch(i, 0.0, None, None, None).is_err());
    assert!(s.add_batch(i, f64::NAN, None, None, None).is_err());
    assert!(s.consume(i, -1.0, None, false).is_err());
    assert!(s
        .add_item(NewItem {
            name: "  ".into(),
            ..Default::default()
        })
        .is_err());
    assert!(s
        .add_item(NewItem {
            name: "Y".into(),
            unit: Some("barrels".into()),
            ..Default::default()
        })
        .is_err());
    assert!(s.add_target("Cash", "other", "weekly", 1.0, "pcs").is_err());
}

#[test]
fn barcode_unique_and_blank_allowed() {
    let s = store();
    item_with(
        &s,
        NewItem {
            name: "A".into(),
            barcode: Some("123".into()),
            ..Default::default()
        },
    );
    assert!(s
        .add_item(NewItem {
            name: "B".into(),
            barcode: Some("123".into()),
            ..Default::default()
        })
        .is_err());
    item_with(
        &s,
        NewItem {
            name: "C".into(),
            barcode: Some("".into()),
            ..Default::default()
        },
    );
    item_with(
        &s,
        NewItem {
            name: "D".into(),
            barcode: Some(" ".into()),
            ..Default::default()
        },
    );
}

#[test]
fn search_matches_name_brand_category() {
    let s = store();
    item_with(
        &s,
        NewItem {
            name: "Basmati".into(),
            category: Some("grains".into()),
            ..Default::default()
        },
    );
    item(&s, "Candles");
    let q = |t: &str| {
        s.items(&ItemQuery {
            search: t,
            ..Default::default()
        })
        .unwrap()
        .len()
    };
    assert_eq!(q("basm"), 1);
    assert_eq!(q("grains"), 1);
    assert_eq!(q("zzz"), 0);
    // LIKE wildcards in user input are just characters to search for, not SQL.
    assert_eq!(q("'; DROP TABLE items; --"), 0);
    assert_eq!(s.all_items().unwrap().len(), 2);
}

#[test]
fn waste_is_logged_separately() {
    let s = store();
    let i = item(&s, "Bread");
    s.add_batch(i, 2.0, None, None, None).unwrap();
    s.consume(i, 1.0, None, true).unwrap();
    let acts: Vec<String> = s
        .history(i, 10)
        .unwrap()
        .into_iter()
        .map(|e| e.action)
        .collect();
    assert_eq!(acts, vec!["waste", "add"]);
}

#[test]
fn undo_consume_restores_batches_and_marks() {
    let s = store();
    let i = item(&s, "Milk");
    let b = s.add_batch(i, 2.0, d(3), None, None).unwrap();
    s.mark_notified(b, &[14, 7]).unwrap();
    let cp = s.checkpoint(i).unwrap();
    s.consume(i, 2.0, None, false).unwrap();
    let t = s.save_undo(cp, "Used 2").unwrap();
    assert!(s.batches(Some(i)).unwrap().is_empty());
    let (id, msg) = s.undo(&t).unwrap();
    assert_eq!((id, msg.as_str()), (Some(i), "Undone: Used 2"));
    let bs = s.batches(Some(i)).unwrap();
    assert_eq!((bs.len(), bs[0].id, bs[0].qty), (1, b, 2.0));
    assert_eq!(
        s.notified_thresholds(b).unwrap().len(),
        2,
        "reminder marks restored"
    );
    assert!(
        !s.history(i, 10).unwrap().iter().any(|e| e.action == "use"),
        "the use event is gone"
    );
    assert_eq!(s.undo(&t).unwrap().1, "Nothing to undo");
}

#[test]
fn undo_refused_after_later_change() {
    let s = store();
    let i = item(&s, "Milk");
    s.add_batch(i, 2.0, None, None, None).unwrap();
    let cp = s.checkpoint(i).unwrap();
    s.consume(i, 1.0, None, false).unwrap();
    let t = s.save_undo(cp, "Used 1").unwrap();
    s.add_batch(i, 5.0, None, None, None).unwrap();
    assert!(s.undo(&t).unwrap().1.starts_with("Can't undo"));
}

#[test]
fn open_batch_shortens_date() {
    let s = store();
    let i = item_with(
        &s,
        NewItem {
            name: "Milk".into(),
            open_days: Some(4),
            ..Default::default()
        },
    );
    let b = s.add_batch(i, 1.0, d(30), None, None).unwrap();
    s.mark_notified(b, &[14]).unwrap();
    assert_eq!(s.open_batch(b, today()).unwrap(), d(4));
    assert!(
        s.notified_thresholds(b).unwrap().is_empty(),
        "reminders restart"
    );
    // Never later than the printed date.
    let b2 = s.add_batch(i, 1.0, d(2), None, None).unwrap();
    assert_eq!(s.open_batch(b2, today()).unwrap(), None);
}

#[test]
fn edit_item_and_barcode_conflicts() {
    let s = store();
    let a = item_with(
        &s,
        NewItem {
            name: "A".into(),
            barcode: Some("111".into()),
            ..Default::default()
        },
    );
    let b = item(&s, "B");
    assert!(s
        .update_item(
            b,
            NewItem {
                name: "B".into(),
                barcode: Some("111".into()),
                ..Default::default()
            }
        )
        .is_err());
    s.update_item(
        b,
        NewItem {
            name: "Bee".into(),
            barcode: Some("222".into()),
            min_qty: Some(3.0),
            ..Default::default()
        },
    )
    .unwrap();
    let it = s.item(b).unwrap().unwrap();
    assert_eq!(
        (it.name.as_str(), it.barcode.as_deref(), it.min_qty),
        ("Bee", Some("222"), 3.0)
    );
    s.update_item(
        a,
        NewItem {
            name: "A".into(),
            barcode: Some("111".into()),
            ..Default::default()
        },
    )
    .unwrap();
}

#[test]
fn locations_cannot_loop() {
    let s = store();
    let a = s.add_location("A", None).unwrap();
    let b = s.add_location("B", Some(a)).unwrap();
    assert!(s.rename_location(a, "A", Some(b)).is_err());
    assert!(s.rename_location(a, "A", Some(a)).is_err());
    s.rename_location(b, "Bee", None).unwrap();
}

#[test]
fn readers_see_writes_at_once_and_work_in_parallel() {
    let dir = tempfile::tempdir().unwrap();
    let s = std::sync::Arc::new(Store::open(dir.path().join("v.db")).unwrap());
    for n in 0..50 {
        let i = item(&s, &format!("Item {n}"));
        s.add_batch(i, 1.0, None, None, None).unwrap();
        // Read straight after the write, through a reader connection.
        assert_eq!(s.item(i).unwrap().unwrap().total, 1.0);
    }
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let s = s.clone();
            std::thread::spawn(move || {
                for _ in 0..20 {
                    assert!(s.all_items().unwrap().len() >= 50);
                }
            })
        })
        .collect();
    let w = s.clone();
    let writer = std::thread::spawn(move || {
        for n in 0..20 {
            item(&w, &format!("Late {n}"));
        }
    });
    for h in handles {
        h.join().unwrap();
    }
    writer.join().unwrap();
    assert_eq!(s.all_items().unwrap().len(), 70);
}

#[test]
fn prices_and_spending() {
    let s = store();
    let i = item(&s, "Coffee");
    s.add_batch(i, 2.0, None, None, Some(9.0)).unwrap();
    s.add_batch(i, 1.0, None, None, None).unwrap();
    s.add_batch(i, 1.0, None, None, Some(5.0)).unwrap();
    let p = s.prices(i, 10).unwrap();
    assert_eq!(p.len(), 2);
    assert_eq!(p[0].price, 5.0);
    assert_eq!(p[1].unit_price, Some(4.5));
    assert_eq!(s.spending("2000-01-01").unwrap(), 14.0);
    assert_eq!(s.spending("2999-01-01").unwrap(), 0.0);
    s.consume(i, 1.0, None, true).unwrap();
    assert_eq!(s.wasted("2000-01-01").unwrap(), 1);
}

#[test]
fn extra_barcodes_find_the_item() {
    let s = store();
    let a = item_with(
        &s,
        NewItem {
            name: "Oat milk".into(),
            barcode: Some("111".into()),
            ..Default::default()
        },
    );
    let b = item(&s, "Rice");
    s.link_barcode("222", a).unwrap();
    assert_eq!(s.item_by_barcode("222").unwrap().unwrap().id, a);
    assert!(s.link_barcode("222", b).is_err(), "already linked");
    assert!(
        s.link_barcode("111", b).is_err(),
        "main barcode of another item"
    );
    assert_eq!(s.extra_barcodes(a).unwrap(), vec!["222"]);
    s.unlink_barcode("222").unwrap();
    assert!(s.item_by_barcode("222").unwrap().is_none());
}

#[test]
fn similar_items_share_a_word() {
    let s = store();
    item(&s, "Haferdrink Barista");
    item(&s, "Oat milk");
    item(&s, "Rice");
    let names: Vec<String> = s
        .similar_items("Barista Haferdrink 1L", 5)
        .unwrap()
        .into_iter()
        .map(|i| i.name)
        .collect();
    assert_eq!(names, vec!["Haferdrink Barista"]);
    assert!(
        s.similar_items("UHT", 5).unwrap().is_empty(),
        "short words don't count"
    );
}

#[test]
fn v2_upgrade_turns_the_people_count_into_adults() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("vorrat.db");
    std::fs::copy("tests/fixtures/schema-v2.db", &p).unwrap();
    {
        let c = rusqlite::Connection::open(&p).unwrap();
        c.execute("UPDATE settings SET value = '5' WHERE key = 'people'", [])
            .unwrap();
    }
    let s = Store::open(&p).unwrap();
    assert_eq!(s.household_counts().unwrap(), (5, 0));
}

#[test]
fn household_members_add_edit_remove() {
    let s = store();
    let kid = s
        .add_member("person", "child", " Mia ", &["vegetarian".into()])
        .unwrap();
    let cat = s.add_member("pet", "cat", "", &[]).unwrap();
    assert_eq!(s.household_counts().unwrap(), (3, 1));
    assert_eq!(s.setting_int("people", 0), 3, "old setting kept in sync");
    let m = s.members().unwrap();
    let mia = m.iter().find(|m| m.id == kid).unwrap();
    assert_eq!(mia.name.as_deref(), Some("Mia"));
    assert_eq!(mia.needs, vec!["vegetarian"]);
    assert!(m.iter().find(|m| m.id == cat).unwrap().name.is_none());

    s.update_member(kid, "teen", "Mia", &[]).unwrap();
    assert!(s
        .members()
        .unwrap()
        .iter()
        .any(|m| m.id == kid && m.r#type == "teen" && m.needs.is_empty()));

    // Wrong combinations are refused.
    assert!(s.add_member("person", "dog", "", &[]).is_err());
    assert!(s.add_member("pet", "cat", "", &["vegan".into()]).is_err());
    assert!(s
        .add_member("person", "adult", "", &["telepathy".into()])
        .is_err());
    assert!(s.add_member("robot", "adult", "", &[]).is_err());
    assert!(
        s.update_member(cat, "teen", "", &[]).is_err(),
        "a pet can't become a teen"
    );

    s.delete_member(cat).unwrap();
    s.delete_member(kid).unwrap();
    assert_eq!(s.household_counts().unwrap(), (2, 0));
    assert_eq!(s.setting_int("people", 0), 2);
}
