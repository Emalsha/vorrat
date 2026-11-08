//! Every read query in `db.rs`, run against a household where each optional
//! column is filled in somewhere and empty somewhere else.
//!
//! The SQL is checked by SQLite only when it runs, and a column read into the
//! wrong Rust type (a NULL into a non-`Option` field, text into a number)
//! fails only on a row that has that value. This test makes sure every reader
//! has met such rows, so a schema or query change that breaks one fails here
//! instead of on someone's page.

mod common;
use common::*;
use vorrat::db::{ItemQuery, NewItem, Sort, EXPORT_TABLES};

/// The demo household, plus rows that fill the optional columns it leaves empty.
#[test]
fn every_reader_handles_full_and_empty_rows() {
    let s = store();
    let today = today();
    vorrat::demo::seed(&s, today).unwrap();

    let id = item_with(
        &s,
        NewItem {
            name: "Oat milk".into(),
            brand: Some("Test".into()),
            barcode: Some("4000000000017".into()),
            category: Some("drinks".into()),
            notes: Some("For porridge".into()),
            ..Default::default()
        },
    );
    s.link_barcode("4000000000024", id).unwrap();
    let kitchen = s.locations().unwrap()[0].id;
    let b1 = s
        .add_batch(id, 2.0, d(3), Some(kitchen), Some(2.5))
        .unwrap();
    s.add_batch(id, 1.0, None, None, None).unwrap();
    s.open_batch(b1, today).unwrap();
    s.consume(id, 0.5, None, false).unwrap();
    s.consume(id, 0.25, None, true).unwrap();
    s.mark_notified(b1, &[7, 3]).unwrap();
    s.mark_low(id).unwrap();
    s.add_attachment(id, "receipt.pdf", "abc123.pdf", "application/pdf", 1234)
        .unwrap();
    s.add_shopping("Oat milk", Some(id), Some(2.0)).unwrap();
    s.add_shopping("Bread", None, None).unwrap();
    s.set_dose(id, None, "liquid", "", 0.1, None).unwrap();
    s.dismiss("test", "2999-01-01").unwrap();
    let cp = s.checkpoint(id).unwrap();
    s.save_undo(cp, "test").unwrap();

    let ts = "1970-01-01";
    assert!(s.members().unwrap().len() >= 4);
    s.household_counts().unwrap();
    assert!(s.doses().unwrap().len() >= 2);
    assert!(s.dose(id).unwrap().is_some());
    s.dose_marks().unwrap();
    s.location_counts().unwrap();
    for sort in [Sort::Name, Sort::Expiry, Sort::Recent] {
        for in_stock_only in [false, true] {
            let q = ItemQuery {
                sort,
                in_stock_only,
                ..Default::default()
            };
            assert!(!s.items(&q).unwrap().is_empty());
        }
    }
    let q = ItemQuery {
        search: "oat",
        kind: "food",
        location_id: Some(kitchen),
        ..Default::default()
    };
    assert_eq!(s.items(&q).unwrap().len(), 1);
    assert!(s.item(id).unwrap().is_some());
    assert!(s.item_by_barcode("4000000000024").unwrap().is_some());
    assert!(s.item_by_name("Oat milk").unwrap().is_some());
    assert_eq!(s.extra_barcodes(id).unwrap().len(), 1);
    s.similar_items("Oat", 5).unwrap();
    assert!(!s.batches(None).unwrap().is_empty());
    assert_eq!(s.batches(Some(id)).unwrap().len(), 2);
    assert!(s.batch(b1).unwrap().is_some());
    assert!(!s.dated_batches().unwrap().is_empty());
    let atts = s.attachments(id).unwrap();
    assert_eq!(atts.len(), 1);
    assert!(s.attachment(atts[0].id).unwrap().is_some());
    assert!(!s.item_files(id).unwrap().is_empty());
    assert!(!s.targets().unwrap().is_empty());
    let shop = s.shopping().unwrap();
    assert_eq!(shop.len(), 2);
    assert!(shop
        .iter()
        .any(|e| e.item_name.as_deref() == Some("Oat milk")));
    assert!(!s.history(id, 50).unwrap().is_empty());
    assert!(!s.recent(50).unwrap().is_empty());
    assert!(!s.item_events(id, "add", ts, 50).unwrap().is_empty());
    s.category_locations("drinks").unwrap();
    assert!(s.dismissed(ts).unwrap().contains("test"));
    let prices = s.prices(id, 10).unwrap();
    assert_eq!(prices.len(), 1);
    assert_eq!(prices[0].unit_price, Some(1.25));
    s.spending(ts).unwrap();
    s.wasted(ts).unwrap();
    assert_eq!(s.notified_thresholds(b1).unwrap().len(), 2);
    assert!(s.low_notified().unwrap().contains(&id));
    let cp = s.checkpoint(id).unwrap();
    assert_eq!(cp["batches"].as_array().unwrap().len(), 2);
    assert_eq!(cp["notified"].as_array().unwrap().len(), 2);
    for t in EXPORT_TABLES {
        s.dump_table(t).unwrap();
    }
}
