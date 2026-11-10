mod common;
use common::*;
use vorrat::jobs::digest::{self, RemindDays};
use vorrat::plan::rules as plan;
use vorrat::stock::queries::NewItem;
use vorrat::stock::rules::{self as stock, Status};

fn rd() -> RemindDays {
    RemindDays::default()
}

#[test]
fn status_buckets() {
    assert_eq!(Status::of(None, 14), Status::None);
    assert_eq!(Status::of(Some(-1), 14), Status::Expired);
    assert_eq!(Status::of(Some(0), 14), Status::Urgent);
    assert_eq!(Status::of(Some(7), 14), Status::Urgent);
    assert_eq!(Status::of(Some(14), 14), Status::Soon);
    assert_eq!(Status::of(Some(15), 14), Status::Ok);
}

#[test]
fn expiring_window() {
    let s = store();
    let i = item(&s, "Milk");
    s.add_batch(i, 1.0, d(3), None, None).unwrap();
    s.add_batch(i, 1.0, d(20), None, None).unwrap();
    s.add_batch(i, 1.0, d(-2), None, None).unwrap();
    let days: Vec<i64> = stock::expiring(s.batches(None).unwrap(), today(), 14)
        .iter()
        .map(|b| b.days_left.unwrap())
        .collect();
    assert_eq!(days, vec![-2, 3]);
}

#[test]
fn low_stock() {
    let s = store();
    let a = item_with(
        &s,
        NewItem {
            name: "Coffee".into(),
            min_qty: Some(2.0),
            ..Default::default()
        },
    );
    s.add_batch(a, 1.0, None, None, None).unwrap();
    let b = item_with(
        &s,
        NewItem {
            name: "Tea".into(),
            min_qty: Some(1.0),
            ..Default::default()
        },
    );
    s.add_batch(b, 3.0, None, None, None).unwrap();
    item(&s, "Salt");
    let names: Vec<String> = stock::low_stock(s.all_items().unwrap())
        .into_iter()
        .map(|i| i.name)
        .collect();
    assert_eq!(names, vec!["Coffee"]);
}

#[test]
fn digest_sends_each_mark_once() {
    let s = store();
    let i = item(&s, "Beans");
    let b = s.add_batch(i, 2.0, d(10), None, None).unwrap();
    let dg = digest::build_digest(&s, today(), &rd()).unwrap();
    assert_eq!(
        dg.soon.iter().map(|x| x.batch.id).collect::<Vec<_>>(),
        vec![b]
    );
    assert!(dg.week.is_empty());
    digest::commit_digest(&s, &dg).unwrap();
    assert!(digest::build_digest(&s, today(), &rd()).unwrap().is_empty());
    let dg2 = digest::build_digest(&s, d(4).unwrap(), &rd()).unwrap();
    assert_eq!(
        dg2.week.iter().map(|x| x.batch.id).collect::<Vec<_>>(),
        vec![b]
    );
    digest::commit_digest(&s, &dg2).unwrap();
    let dg3 = digest::build_digest(&s, d(11).unwrap(), &rd()).unwrap();
    assert_eq!(
        dg3.expired.iter().map(|x| x.batch.id).collect::<Vec<_>>(),
        vec![b]
    );
}

#[test]
fn digest_skips_marks_already_passed() {
    let s = store();
    let i = item(&s, "Bread");
    let b = s.add_batch(i, 1.0, d(3), None, None).unwrap();
    let dg = digest::build_digest(&s, today(), &rd()).unwrap();
    assert_eq!(dg.week.len(), 1);
    assert!(dg.soon.is_empty());
    digest::commit_digest(&s, &dg).unwrap();
    assert_eq!(
        s.notified_thresholds(b).unwrap(),
        [14, 7].into_iter().collect()
    );
}

#[test]
fn documents_use_longer_marks() {
    let s = store();
    let p = item_with(
        &s,
        NewItem {
            name: "Passport".into(),
            kind: Some("document".into()),
            ..Default::default()
        },
    );
    s.add_batch(p, 1.0, d(60), None, None).unwrap();
    let dg = digest::build_digest(&s, today(), &rd()).unwrap();
    assert_eq!(
        dg.soon
            .iter()
            .map(|x| x.batch.item_name.as_str())
            .collect::<Vec<_>>(),
        vec!["Passport"]
    );
}

#[test]
fn low_stock_alerts_once_until_restocked() {
    let s = store();
    let i = item_with(
        &s,
        NewItem {
            name: "Coffee".into(),
            min_qty: Some(2.0),
            ..Default::default()
        },
    );
    s.add_batch(i, 1.0, None, None, None).unwrap();
    let dg = digest::build_digest(&s, today(), &rd()).unwrap();
    assert_eq!(dg.low.len(), 1);
    digest::commit_digest(&s, &dg).unwrap();
    assert!(digest::build_digest(&s, today(), &rd()).unwrap().is_empty());
    s.add_batch(i, 5.0, None, None, None).unwrap();
    digest::build_digest(&s, today(), &rd()).unwrap();
    s.consume(i, 5.0, None, false).unwrap();
    assert_eq!(
        digest::build_digest(&s, today(), &rd()).unwrap().low.len(),
        1
    );
}

#[test]
fn digest_text() {
    let s = store();
    let i = item_with(
        &s,
        NewItem {
            name: "Milk".into(),
            unit: Some("L".into()),
            ..Default::default()
        },
    );
    s.add_batch(i, 1.0, d(2), None, None).unwrap();
    let dg = digest::build_digest(&s, today(), &rd()).unwrap();
    assert_eq!(dg.title(), "Vorrat: 1 expiring this week");
    assert!(dg.body().contains("Milk (1 L, in 2d)"), "{}", dg.body());
}

/// Twelve 1.5 L bottles count as 18 L and 500 ml of juice as 0.5 L; expired
/// stock doesn't count.
#[test]
fn coverage_units_and_sizes() {
    let s = store();
    let water = item_with(
        &s,
        NewItem {
            name: "Water 1.5L".into(),
            category: Some("water".into()),
            size: Some(1.5),
            size_unit: Some("L".into()),
            ..Default::default()
        },
    );
    s.add_batch(water, 12.0, None, None, None).unwrap();
    let juice = item_with(
        &s,
        NewItem {
            name: "Juice".into(),
            category: Some("water".into()),
            unit: Some("ml".into()),
            ..Default::default()
        },
    );
    s.add_batch(juice, 500.0, None, None, None).unwrap();
    let old = item_with(
        &s,
        NewItem {
            name: "Old".into(),
            category: Some("water".into()),
            unit: Some("L".into()),
            ..Default::default()
        },
    );
    s.add_batch(old, 10.0, d(-1), None, None).unwrap();
    let plan = s.plan(today()).unwrap();
    let w = plan
        .lines
        .iter()
        .find(|c| c.target.category == "water")
        .unwrap();
    assert_eq!((w.need, w.have), (56.0, 18.5));
    assert_eq!(w.days, Some(4.6));
    assert_eq!(w.items.len(), 2);
}

#[test]
fn coverage_fixed_targets() {
    let s = store();
    let bat = item_with(
        &s,
        NewItem {
            name: "AA".into(),
            category: Some("batteries".into()),
            ..Default::default()
        },
    );
    s.add_batch(bat, 8.0, None, None, None).unwrap();
    let plan = s.plan(today()).unwrap();
    let c = plan
        .lines
        .iter()
        .find(|c| c.target.category == "batteries")
        .unwrap();
    assert_eq!((c.need, c.have, c.missing), (12.0, 8.0, 4.0));
    assert_eq!(c.days, None);
}

#[test]
fn coverage_ignores_dimension_mismatch() {
    let s = store();
    let tins = item_with(
        &s,
        NewItem {
            name: "Peas".into(),
            category: Some("vegetables".into()),
            ..Default::default()
        },
    );
    s.add_batch(tins, 5.0, None, None, None).unwrap();
    let plan = s.plan(today()).unwrap();
    assert_eq!(
        plan.lines
            .iter()
            .find(|c| c.target.category == "vegetables")
            .unwrap()
            .have,
        0.0
    );
}

/// Vegetables etc. are at 0 days, so the weakest is 0.
#[test]
fn weakest_days_follows_the_scarcest_essential() {
    let s = store();
    let first = s.members().unwrap()[0].id;
    s.delete_member(first).unwrap();
    for (cat, unit, qty) in [("water", "L", 20.0), ("grains", "kg", 0.7)] {
        let i = item_with(
            &s,
            NewItem {
                name: cat.into(),
                category: Some(cat.into()),
                unit: Some(unit.into()),
                ..Default::default()
            },
        );
        s.add_batch(i, qty, None, None, None).unwrap();
    }
    assert_eq!(s.plan(today()).unwrap().weakest_days, Some(0.0));
}

#[test]
fn fmt_qty_matches_python_g() {
    assert_eq!(stock::fmt_qty(1.0), "1");
    assert_eq!(stock::fmt_qty(0.5), "0.5");
    assert_eq!(stock::fmt_qty(18.5), "18.5");
    assert_eq!(stock::fmt_qty(0.036), "0.036");
}

#[test]
fn power_cut_eats_the_fridge_first() {
    use vorrat::plan::rules::{cold_kind, power_cut, Cold};
    assert_eq!(cold_kind("Tiefkühltruhe"), Some(Cold::Freezer));
    assert_eq!(cold_kind("Kühlschrank"), Some(Cold::Fridge));
    assert_eq!(cold_kind("Kitchen"), None);
    let s = common::store();
    let fridge = s.add_location("My fridge", None).unwrap();
    let freezer = s.add_location("Chest freezer", None).unwrap();
    let drawer = s.add_location("Drawer 2", Some(freezer)).unwrap();
    let pantry = s.add_location("Shelf", None).unwrap();
    let milk = common::item(&s, "Milk");
    let peas = common::item(&s, "Peas");
    let rice = common::item(&s, "Rice");
    s.add_batch(milk, 1.0, common::d(5), Some(fridge), None)
        .unwrap();
    s.add_batch(milk, 1.0, common::d(2), Some(fridge), None)
        .unwrap();
    s.add_batch(peas, 1.0, common::d(200), Some(drawer), None)
        .unwrap();
    s.add_batch(rice, 1.0, common::d(300), Some(pantry), None)
        .unwrap();
    let crackers = common::item(&s, "Crackers");
    s.add_batch(crackers, 1.0, common::d(90), Some(pantry), None)
        .unwrap();
    s.add_batch(crackers, 1.0, common::d(-1), Some(pantry), None)
        .unwrap();
    let no_cook = std::collections::HashSet::from([crackers, milk]);
    let pc = power_cut(
        &s.locations().unwrap(),
        s.batches(None).unwrap(),
        &no_cook,
        common::today(),
    );
    assert_eq!(pc.fridge.len(), 2);
    assert_eq!(pc.fridge[0].days_left, Some(2), "soonest first");
    assert_eq!(pc.freezer.len(), 1, "a drawer inside the freezer counts");
    assert_eq!(pc.freezer[0].batch.item_name, "Peas");
    assert_eq!(
        pc.no_cook.len(),
        1,
        "only unexpired no-cook food outside the cold, not rice"
    );
    assert_eq!(pc.no_cook[0].batch.item_name, "Crackers");
}

/// Only food marked as needing no cooking counts towards the days without
/// cooking; lines without any have none.
#[test]
fn plan_counts_days_without_cooking() {
    let s = store();
    let rice = item(&s, "Rice");
    let bread = item(&s, "Crispbread");
    for (id, name, no_cook) in [(rice, "Rice", false), (bread, "Crispbread", true)] {
        s.update_item(
            id,
            NewItem {
                name: name.into(),
                category: Some("grains".into()),
                unit: Some("kg".into()),
                no_cook,
                ..Default::default()
            },
        )
        .unwrap();
    }
    s.add_batch(rice, 7.0, None, None, None).unwrap();
    s.add_batch(bread, 1.4, None, None, None).unwrap();
    assert!(s.item(bread).unwrap().unwrap().no_cook);
    let p = s.plan(today()).unwrap();
    let grains = p
        .lines
        .iter()
        .find(|c| c.target.category == "grains")
        .unwrap();
    assert_eq!((grains.have, grains.no_cook), (8.4, 1.4));
    assert_eq!(grains.days, Some(12.0));
    assert_eq!(grains.no_cook_days, Some(2.0));
    let water = p
        .lines
        .iter()
        .find(|c| c.target.category == "water")
        .unwrap();
    assert_eq!(water.no_cook_days, None);
}

/// The store starts with two adults and 14 days.
#[test]
fn plan_scales_with_people_and_pets() {
    let s = store();
    s.set_setting("days", "10").unwrap();
    s.add_member("person", "baby", "", &[]).unwrap();
    s.add_target("Pet food", "pet", "per_pet_day", 0.1, "kg")
        .unwrap();
    s.add_target("Blanket", "other", "per_person", 1.0, "pcs")
        .unwrap();
    s.add_target("Radio", "tools", "fixed", 1.0, "pcs").unwrap();
    let line = |p: &plan::Plan, label: &str| {
        p.lines
            .iter()
            .find(|c| c.target.label == label)
            .unwrap()
            .clone()
    };
    let p = s.plan(today()).unwrap();
    assert_eq!((p.people, p.pets), (3, 0));
    assert_eq!(line(&p, "Drinking water & drinks").need, 2.0 * 3.0 * 10.0);
    assert_eq!(line(&p, "Pet food").need, 0.0, "no pets, nothing needed");
    assert_eq!(line(&p, "Pet food").days, None);
    assert_eq!(line(&p, "Blanket").need, 3.0);
    assert_eq!(line(&p, "Radio").need, 1.0);

    s.add_member("pet", "dog", "Rex", &[]).unwrap();
    s.add_member("pet", "cat", "", &[]).unwrap();
    let food = item_with(
        &s,
        NewItem {
            name: "Dry food".into(),
            category: Some("pet".into()),
            unit: Some("kg".into()),
            ..Default::default()
        },
    );
    s.add_batch(food, 1.0, None, None, None).unwrap();
    let p = s.plan(today()).unwrap();
    assert_eq!(p.pets, 2);
    let pf = line(&p, "Pet food");
    assert_eq!(pf.need, 2.0);
    assert_eq!(pf.days, Some(5.0), "1 kg for two pets at 0.1 kg each a day");
    assert_eq!(p.weakest_days, Some(0.0));
}
