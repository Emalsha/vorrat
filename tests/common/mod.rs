#![allow(dead_code)]
use chrono::{Days, NaiveDate};
use vorrat::db::{NewItem, Store};

pub fn today() -> NaiveDate {
    vorrat::db::today()
}

/// `n` days from today (negative = in the past).
pub fn d(n: i64) -> Option<NaiveDate> {
    let t = today();
    Some(if n >= 0 {
        t + Days::new(n as u64)
    } else {
        t - Days::new((-n) as u64)
    })
}

pub fn store() -> Store {
    Store::open_in_memory().unwrap()
}

pub fn item(s: &Store, name: &str) -> i64 {
    s.add_item(NewItem {
        name: name.into(),
        ..Default::default()
    })
    .unwrap()
}

pub fn item_with(s: &Store, f: NewItem) -> i64 {
    s.add_item(f).unwrap()
}
