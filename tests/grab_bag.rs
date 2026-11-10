//! Grab bags: a place per person, ticked by unexpired stock in it or in a
//! place inside it, and every entry translated.

mod common;

use std::collections::HashSet;

use vorrat::grab_bag::rules::{check, inside, ENTRIES};
use vorrat::i18n::Lang;
use vorrat::stock::queries::NewItem;
use vorrat::stock::rules::annotate;

#[test]
fn every_entry_is_translated() {
    for e in ENTRIES {
        assert!(Lang::De.table().contains_key(e.label), "{}", e.label);
    }
}

#[test]
fn stock_in_the_bag_ticks_its_entries() {
    let s = common::store();
    let bag = s.add_location("Bag", None).unwrap();
    let pocket = s.add_location("Pocket", Some(bag)).unwrap();
    let kitchen = s.add_location("Kitchen shelf", None).unwrap();
    let thing = |name: &str, category: &str| {
        common::item_with(
            &s,
            NewItem {
                name: name.into(),
                category: Some(category.into()),
                ..Default::default()
            },
        )
    };
    s.add_batch(
        thing("Passport copy", "documents"),
        1.0,
        None,
        Some(pocket),
        None,
    )
    .unwrap();
    s.add_batch(
        thing("Bottle", "water"),
        2.0,
        common::d(-1),
        Some(bag),
        None,
    )
    .unwrap();
    s.add_batch(thing("Torch", "light"), 1.0, None, Some(kitchen), None)
        .unwrap();
    let locs = s.locations().unwrap();
    let places = inside(bag, &locs);
    assert_eq!(places, HashSet::from([bag, pocket]));
    let batches = annotate(s.batches(None).unwrap(), common::today());
    let lines = check(&places, &batches);
    let line = |label: &str| lines.iter().find(|l| l.entry.label == label).unwrap();
    assert!(
        line("Copies of documents").ok(),
        "a place inside the bag counts"
    );
    assert!(
        !line("Water").ok() && line("Water").expired(),
        "expired water doesn't count"
    );
    assert!(!line("Torch").ok(), "the kitchen torch is not in the bag");
}
