mod common;
use vorrat::transfer::{export_csv, export_json, import_csv, parse_csv};

#[test]
fn parses_quotes_semicolons_and_bom() {
    let rows = parse_csv("\u{feff}name;qty\r\n\"Käse; alt\";2,5\n\n\"say \"\"hi\"\"\nthere\";1");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1], vec!["Käse; alt", "2,5"]);
    assert_eq!(rows[2][0], "say \"hi\"\nthere");
}

#[test]
fn import_creates_and_matches_items() {
    let s = common::store();
    let rice = common::item(&s, "Rice");
    let csv = "Name;Qty;Unit;Category;Expires_on;Location;Barcode\n\
               rice;2;kg;grains;01.05.2030;Garage shelf;\n\
               Water;6;L;water;2030-01-01;Garage shelf;4001\n\
               ;1;;;;;\n\
               Beans;x;;;;;\n\
               Tea;1;;;31.02.2030;;\n\
               Candles;0;pcs;light;;;\n";
    let r = import_csv(&s, csv).unwrap();
    assert_eq!(r.rows, 3, "{r:?}");
    assert_eq!(r.new_items, 2);
    assert_eq!(r.batches, 2);
    assert_eq!(r.skipped.len(), 3);
    assert_eq!(s.item(rice).unwrap().unwrap().total, 2.0);
    let w = s.item_by_barcode("4001").unwrap().unwrap();
    assert_eq!(w.total, 6.0);
    let before = common::store().locations().unwrap().len();
    assert_eq!(
        s.locations().unwrap().len(),
        before + 1,
        "Garage shelf created once"
    );
    assert!(r.summary().contains("line 4"));
    // Importing again adds stock, never new duplicates.
    let r = import_csv(&s, csv).unwrap();
    assert_eq!(r.new_items, 0);
    assert_eq!(s.item(rice).unwrap().unwrap().total, 4.0);
}

#[test]
fn import_needs_a_name_column() {
    let s = common::store();
    assert!(import_csv(&s, "qty,unit\n1,kg\n").is_err());
    assert!(import_csv(&s, "").is_err());
}

#[test]
fn csv_round_trips_and_is_safe_in_spreadsheets() {
    let a = common::store();
    let id = common::item(&a, "=HYPERLINK(\"x\")");
    a.add_batch(id, 3.0, common::d(30), None, Some(1.5))
        .unwrap();
    common::item(&a, "Salt, coarse");
    let csv = export_csv(&a).unwrap();
    assert!(csv.contains("\"'=HYPERLINK(\"\"x\"\")\""), "{csv}");
    assert!(csv.contains("\"Salt, coarse\""));

    let b = common::store();
    let r = import_csv(&b, &csv).unwrap();
    assert_eq!(r.new_items, 2, "{r:?}");
    assert_eq!(
        b.item_by_name("=HYPERLINK(\"x\")").unwrap().unwrap().total,
        3.0
    );
}

#[test]
fn json_export_has_every_table() {
    let s = common::store();
    common::item(&s, "Oats");
    let v = export_json(&s, "2026-11-07T10:00:00").unwrap();
    assert_eq!(v["format"], "vorrat-export");
    assert_eq!(v["tables"]["items"][0]["name"], "Oats");
    assert!(v["tables"]["targets"].as_array().unwrap().len() >= 5);
    assert!(v["tables"].get("undo").is_none());
}
