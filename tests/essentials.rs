mod common;
use common::*;
use vorrat::db::NewItem;
use vorrat::essentials::{self as es, checklist, medicines};
use vorrat::rules::{self, RemindDays, Snapshot};

fn snap(s: &vorrat::db::Store) -> Snapshot {
    Snapshot::load(s, today()).unwrap()
}

fn adult(s: &vorrat::db::Store) -> i64 {
    s.members().unwrap()[0].id
}

#[test]
fn medicine_dose_add_edit_remove() {
    let s = store();
    let who = adult(&s);
    let id = s
        .add_medicine("Ramipril", "pcs", Some(who), "tablet", "5 mg", 2.0, d(30))
        .unwrap();
    let it = s.item(id).unwrap().unwrap();
    assert_eq!(
        (it.category.as_deref(), it.kind.as_str()),
        (Some("medicine"), "supply")
    );
    let dose = s.dose(id).unwrap().unwrap();
    assert_eq!((dose.member_id, dose.per_day), (Some(who), 2.0));
    assert_eq!(dose.strength, "5 mg");

    // Bad input adds nothing.
    let before = s.all_items().unwrap().len();
    assert!(s
        .add_medicine("X", "pcs", None, "potion", "", 1.0, None)
        .is_err());
    assert!(s.add_medicine("X", "pcs", None, "", "", 0.0, None).is_err());
    assert!(s
        .add_medicine("X", "pcs", Some(9999), "", "", 1.0, None)
        .is_err());
    assert_eq!(s.all_items().unwrap().len(), before);
    assert!(s.set_dose(9999, None, "", "", 1.0, None).is_err());

    s.set_dose(id, None, "", "", 1.0, None).unwrap();
    assert_eq!(s.dose(id).unwrap().unwrap().member_id, None);

    // Removing the person keeps the medicine, unassigned.
    s.set_dose(id, Some(who), "tablet", "", 1.0, None).unwrap();
    s.delete_member(who).unwrap();
    assert_eq!(s.dose(id).unwrap().unwrap().member_id, None);

    s.delete_dose(id).unwrap();
    assert!(s.dose(id).unwrap().is_none());
    assert!(s.item(id).unwrap().is_some(), "the item stays");

    s.set_dose(id, None, "", "", 1.0, None).unwrap();
    s.delete_item(id).unwrap();
    assert!(
        s.doses().unwrap().is_empty(),
        "deleting the item drops the dose"
    );
}

#[test]
fn medicine_days_count_usable_stock_and_join_the_plan() {
    let s = store();
    let id = s
        .add_medicine("Inhaler", "pcs", None, "inhaler", "", 2.0, None)
        .unwrap();
    s.add_batch(id, 10.0, d(100), None, None).unwrap();
    s.add_batch(id, 50.0, d(-1), None, None).unwrap(); // expired: doesn't count
    let m = medicines(&s, &snap(&s)).unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!((m[0].have, m[0].days), (10.0, 5.0));
    let plan = rules::coverage(&s, today()).unwrap();
    assert_eq!(plan.medicine_days, Some(5.0));
    assert_eq!(
        plan.weakest_days,
        Some(0.0),
        "nothing else stocked yet, so food is weakest"
    );
    // Without a medicine the plan has no medicine days.
    s.delete_dose(id).unwrap();
    assert_eq!(rules::coverage(&s, today()).unwrap().medicine_days, None);
}

#[test]
fn medicine_reminders_once_until_restocked() {
    let s = store();
    let who = adult(&s);
    let id = s
        .add_medicine("Insulin", "ml", Some(who), "injection", "", 1.0, d(10))
        .unwrap();
    s.add_batch(id, 5.0, None, None, None).unwrap();
    let dg = rules::build_digest(&s, today(), &RemindDays::default()).unwrap();
    assert_eq!(dg.medicine.len(), 1);
    assert_eq!(dg.renew.len(), 1);
    assert_eq!(dg.medicine[0].who.as_deref(), Some("Adult 1"));
    assert!(
        dg.title().contains("1 medicine running out"),
        "{}",
        dg.title()
    );
    assert!(
        dg.body().contains("Insulin (Adult 1, 5 days left)"),
        "{}",
        dg.body()
    );
    assert!(dg
        .body()
        .contains("Prescription to renew: Insulin (Adult 1, in 10d)"));
    rules::commit_digest(&s, &dg).unwrap();
    assert!(rules::build_digest(&s, today(), &RemindDays::default())
        .unwrap()
        .is_empty());

    // Restocked: quiet, and the next shortage alerts again.
    s.add_batch(id, 100.0, None, None, None).unwrap();
    assert!(rules::build_digest(&s, today(), &RemindDays::default())
        .unwrap()
        .medicine
        .is_empty());
    s.consume(id, 100.0, None, false).unwrap();
    assert_eq!(
        rules::build_digest(&s, today(), &RemindDays::default())
            .unwrap()
            .medicine
            .len(),
        1
    );
}

#[test]
fn checklists_tick_from_linked_stock() {
    let s = store();
    let t = s
        .add_item(NewItem {
            name: "Thermometer".into(),
            category: Some("first aid".into()),
            ..Default::default()
        })
        .unwrap();
    s.set_preset(t, Some("fa-thermometer")).unwrap();
    assert!(s.set_preset(t, Some("fa-unicorn")).is_err());
    let state = |s: &vorrat::db::Store, list, key: &str| {
        checklist(list, &snap(s), 2)
            .into_iter()
            .find(|c| c.check.key == key)
            .unwrap()
    };
    assert!(
        !state(&s, es::FIRST_AID, "fa-thermometer").ok(),
        "no stock yet"
    );
    s.add_batch(t, 1.0, None, None, None).unwrap();
    assert!(state(&s, es::FIRST_AID, "fa-thermometer").ok());

    // Expired stock doesn't count, and is flagged.
    let p = item(&s, "Plasters");
    s.set_preset(p, Some("fa-plasters")).unwrap();
    s.add_batch(p, 20.0, d(-3), None, None).unwrap();
    let c = state(&s, es::FIRST_AID, "fa-plasters");
    assert!(!c.ok() && c.expired);

    // Blankets: one per person.
    let b = item(&s, "Wool blanket");
    s.set_preset(b, Some("es-blankets")).unwrap();
    s.add_batch(b, 1.0, None, None, None).unwrap();
    let c = state(&s, es::ESSENTIALS, "es-blankets");
    assert_eq!((c.have, c.need, c.ok()), (1.0, 2.0, false));
    s.set_preset(b, None).unwrap();
    assert!(state(&s, es::ESSENTIALS, "es-blankets").items.is_empty());
}

#[test]
fn checklist_keys_are_unique_and_sourced() {
    let mut keys = std::collections::HashSet::new();
    for c in es::FIRST_AID.iter().chain(es::ESSENTIALS) {
        assert!(keys.insert(c.key), "duplicate key {}", c.key);
        assert!(!c.named_by.is_empty(), "{} names no source", c.key);
        assert!(
            vorrat::db::CATEGORIES.contains(&c.category),
            "{}: unknown category",
            c.key
        );
    }
    // Every country named has a verified entry in the guidance data.
    let data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string("docs/guidance/presets.json").unwrap())
            .unwrap();
    let verified: std::collections::HashSet<&str> = data["sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["status"] == "verified")
        .filter_map(|s| s["code"].as_str())
        .collect();
    for c in es::FIRST_AID.iter().chain(es::ESSENTIALS) {
        for code in c.named_by {
            assert!(verified.contains(code), "{}: {code} is not verified", c.key);
        }
    }
}

#[test]
fn export_marks_health_data_private() {
    let s = store();
    let plain = vorrat::transfer::export_json(&s, "now").unwrap();
    assert!(plain.get("private").is_none());
    assert_eq!(plain["schema"], 4);
    s.add_medicine("Drops", "ml", None, "drops", "", 1.0, None)
        .unwrap();
    let doc = vorrat::transfer::export_json(&s, "now").unwrap();
    assert!(doc["private"].as_str().unwrap().contains("medicines"));
    assert_eq!(doc["tables"]["doses"].as_array().unwrap().len(), 1);
}
