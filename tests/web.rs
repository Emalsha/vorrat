mod common;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use vorrat::web::{router, AppState};

fn app() -> (axum::Router, Arc<AppState>) {
    let cfg = vorrat::config::Config::from_lookup(|k| (k == "VORRAT_LOOKUP").then(|| "off".into()))
        .unwrap();
    let st = Arc::new(AppState::new(common::store(), cfg));
    (router(st.clone()), st)
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, String) {
    let r = app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let s = r.status();
    (
        s,
        String::from_utf8(r.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap(),
    )
}

async fn post(app: &axum::Router, uri: &str, form: &str, site: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::post(uri)
                .header("content-type", "application/x-www-form-urlencoded")
                .header("sec-fetch-site", site)
                .body(Body::from(form.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn every_page_renders() {
    let (app, _) = app();
    for p in [
        "/",
        "/items",
        "/add",
        "/plan",
        "/shopping",
        "/settings",
        "/scan",
        "/scan?mode=use",
        "/static/scan.js",
        "/static/print.js",
        "/static/app.js",
        "/sw.js",
        "/plan/print",
        "/plan/power-cut",
        "/essentials",
        "/static/zxing-0.23.0.min.js",
        "/healthz",
        "/static/app.css",
    ] {
        let (s, _) = get(&app, p).await;
        assert_eq!(s, StatusCode::OK, "{p}");
    }
    assert_eq!(get(&app, "/items/999").await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        get(&app, "/static/../Cargo.toml").await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn add_then_see_it_everywhere() {
    let (app, st) = app();
    let r = post(
        &app,
        "/add",
        "name=Still+water&category=water&unit=pcs&qty=6&size=1%2C5&size_unit=L",
        "same-origin",
    )
    .await;
    assert_eq!(r.status(), StatusCode::SEE_OTHER);
    let it = &st.store.all_items().unwrap()[0];
    assert_eq!(
        (it.total, it.size),
        (6.0, Some(1.5)),
        "decimal comma accepted"
    );
    let (_, body) = get(&app, "/plan").await;
    assert!(body.contains("9 / 56 L"), "water coverage shown");
    let (_, body) = get(&app, &format!("/items/{}", it.id)).await;
    assert!(body.contains("Still water"));
}

#[tokio::test]
async fn known_barcode_adds_to_existing_item() {
    let (app, st) = app();
    post(&app, "/add", "name=Rice&barcode=123&qty=1", "same-origin").await;
    post(
        &app,
        "/add",
        "name=Rice+again&barcode=123&qty=2",
        "same-origin",
    )
    .await;
    let items = st.store.all_items().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].total, 3.0);
}

#[tokio::test]
async fn cross_site_posts_are_refused() {
    let (app, st) = app();
    let r = post(&app, "/add", "name=Evil", "cross-site").await;
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    assert!(st.store.all_items().unwrap().is_empty());
}

#[tokio::test]
async fn output_is_escaped_and_headers_are_strict() {
    let (app, _) = app();
    post(
        &app,
        "/add",
        "name=%3Cscript%3Ealert(1)%3C%2Fscript%3E",
        "same-origin",
    )
    .await;
    let r = app
        .clone()
        .oneshot(Request::get("/items").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let csp = r
        .headers()
        .get("content-security-policy")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(csp.contains("script-src 'self'"));
    assert_eq!(r.headers().get("x-frame-options").unwrap(), "DENY");
    let body =
        String::from_utf8(r.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(!body.contains("<script>alert"));
    assert!(body.contains("&#60;script&#62;") || body.contains("&lt;script&gt;"));
}

#[tokio::test]
async fn open_redirects_are_not_possible() {
    let (app, st) = app();
    let id = common::item(&st.store, "Tea");
    st.store.add_batch(id, 1.0, None, None, None).unwrap();
    let r = post(
        &app,
        &format!("/items/{id}/use"),
        "qty=1&back=%2F%2Fevil.example",
        "same-origin",
    )
    .await;
    let loc = r.headers().get("location").unwrap().to_str().unwrap();
    assert!(loc.starts_with(&format!("/items/{id}")), "{loc}");
}

#[tokio::test]
async fn plan_settings_and_gaps_to_shopping() {
    let (app, st) = app();
    post(&app, "/plan/settings", "days=10", "same-origin").await;
    assert_eq!(st.store.setting_int("days", 0), 10);
    let r = post(&app, "/plan/settings", "days=0", "same-origin").await;
    assert!(
        r.headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("Not+saved")
            || r.headers()
                .get("location")
                .unwrap()
                .to_str()
                .unwrap()
                .contains("Not%20saved")
    );
    post(&app, "/plan/to-shopping", "", "same-origin").await;
    let n = st.store.shopping().unwrap().len();
    assert_eq!(n, 10, "every target is a gap on an empty stock");
    post(&app, "/plan/to-shopping", "", "same-origin").await;
    assert_eq!(st.store.shopping().unwrap().len(), n, "no duplicates");
}

#[tokio::test]
async fn api_returns_json() {
    let (app, st) = app();
    let id = common::item(&st.store, "Tea");
    st.store
        .add_batch(id, 1.0, common::d(3), None, None)
        .unwrap();
    let (s, body) = get(&app, "/api/v1/plan").await;
    assert_eq!(s, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["people"], 2);
    let (_, body) = get(&app, "/api/v1/expiring").await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v[0]["item_name"], "Tea");
    assert_eq!(v[0]["status"], "urgent");
    let (_, body) = get(&app, "/api/v1/digest").await;
    assert!(body.contains("expiring this week"));
}

#[tokio::test]
async fn settings_never_reveal_the_ntfy_topic() {
    let cfg = vorrat::config::Config::from_lookup(|k| {
        (k == "VORRAT_NTFY_TOPIC").then(|| "secret-topic-123".into())
    })
    .unwrap();
    let app = router(Arc::new(AppState::new(common::store(), cfg)));
    let (s, body) = get(&app, "/settings").await;
    assert_eq!(s, StatusCode::OK);
    assert!(body.contains("ntfy.sh"));
    assert!(!body.contains("secret-topic-123"));
}

#[tokio::test]
async fn test_push_without_topic_explains() {
    let (app, _) = app();
    let r = post(&app, "/settings/test-push", "", "same-origin").await;
    assert!(r
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("VORRAT_NTFY_TOPIC"));
}

fn loc(r: &axum::response::Response) -> String {
    r.headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string()
}

async fn get_resp(app: &axum::Router, uri: &str) -> axum::response::Response {
    app.clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn scanning_a_known_code() {
    let (app, st) = app();
    post(
        &app,
        "/add",
        "name=Rice&barcode=4000000000001&qty=2",
        "same-origin",
    )
    .await;
    let id = st.store.all_items().unwrap()[0].id;
    let r = get_resp(&app, "/scan/code?code=4000000000001").await;
    assert_eq!(loc(&r), format!("/items/{id}"));
    let r = get_resp(&app, "/scan/code?code=4000000000001&mode=use").await;
    assert!(loc(&r).contains("Used"));
    assert_eq!(st.store.item(id).unwrap().unwrap().total, 1.0);
}

#[tokio::test]
async fn scanning_an_unknown_code_prefills_the_form() {
    let (app, _) = app();
    let r = get_resp(&app, "/scan/code?code=4000000000999").await;
    let to = loc(&r);
    assert!(to.starts_with("/add?barcode=4000000000999"), "{to}");
    let (s, body) = get(&app, &to).await;
    assert_eq!(s, StatusCode::OK);
    assert!(body.contains("value=\"4000000000999\""));
    let r = get_resp(&app, "/scan/code?code=4000000000999&mode=use").await;
    assert!(loc(&r).contains("not+in") || loc(&r).contains("not%20in"));
}

/// A forged image name in the form is dropped, not stored.
#[tokio::test]
async fn files_cannot_escape_the_folder() {
    let (app, st) = app();
    for p in [
        "/files/..%2Fvorrat.db",
        "/files/vorrat.db",
        "/files/img-0123456789abcdef0123456789abcdef.jpg",
    ] {
        assert_eq!(get(&app, p).await.0, StatusCode::NOT_FOUND, "{p}");
    }
    post(
        &app,
        "/add",
        "name=X&image=..%2F..%2Fetc%2Fpasswd",
        "same-origin",
    )
    .await;
    assert_eq!(st.store.all_items().unwrap()[0].image, None);
    let (_, body) = get(&app, "/add?name=Y&image=..%2Fsecret").await;
    assert!(!body.contains("secret"));
}

#[tokio::test]
async fn scan_page_has_no_inline_script() {
    let (app, _) = app();
    let (_, body) = get(&app, "/scan").await;
    assert!(body.contains("/static/scan.js"));
    assert!(
        !body.contains("<script>"),
        "inline scripts would be blocked by the CSP"
    );
}

#[tokio::test]
async fn use_then_undo_through_the_ui() {
    let (app, st) = app();
    let id = common::item(&st.store, "Tea");
    st.store.add_batch(id, 2.0, None, None, None).unwrap();
    let r = post(&app, &format!("/items/{id}/use"), "qty=2", "same-origin").await;
    let to = loc(&r);
    assert!(to.contains("undo="), "{to}");
    let (_, page) = get(&app, &to).await;
    assert!(page.contains("/undo/"));
    let token = to.split("undo=").nth(1).unwrap();
    post(&app, &format!("/undo/{token}"), "", "same-origin").await;
    assert_eq!(st.store.item(id).unwrap().unwrap().total, 2.0);
}

#[tokio::test]
async fn edit_item_page_saves() {
    let (app, st) = app();
    let id = common::item(&st.store, "Tea");
    assert_eq!(
        get(&app, &format!("/items/{id}/edit")).await.0,
        StatusCode::OK
    );
    let r = post(
        &app,
        &format!("/items/{id}/edit"),
        "name=Green+tea&kind=food&unit=g&category=drinks&open_days=30",
        "same-origin",
    )
    .await;
    assert_eq!(loc(&r), format!("/items/{id}?msg=Saved"));
    let it = st.store.item(id).unwrap().unwrap();
    assert_eq!(
        (it.name.as_str(), it.unit.as_str(), it.open_days),
        ("Green tea", "g", Some(30))
    );
}

fn multipart(name: &str, bytes: &[u8]) -> (String, Vec<u8>) {
    let b = "XBOUNDARYX";
    let mut body = format!("--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/pdf\r\n\r\n").into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{b}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={b}"), body)
}

/// Deleting the item removes its files from disk.
#[tokio::test]
async fn attachments_only_accept_real_pdfs_and_images() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = vorrat::config::Config::from_lookup(|k| match k {
        "VORRAT_DATA_DIR" => Some(dir.path().to_string_lossy().into()),
        "VORRAT_LOOKUP" => Some("off".into()),
        _ => None,
    })
    .unwrap();
    let st = Arc::new(AppState::new(common::store(), cfg));
    let app = router(st.clone());
    let id = common::item(&st.store, "Passport");
    let send = |name: &str, bytes: &[u8]| {
        let (ct, body) = multipart(name, bytes);
        app.clone().oneshot(
            Request::post(format!("/items/{id}/attachments"))
                .header("content-type", ct)
                .header("sec-fetch-site", "same-origin")
                .body(Body::from(body))
                .unwrap(),
        )
    };
    let r = send("evil.pdf", b"<html><script>alert(1)</script>")
        .await
        .unwrap();
    assert!(loc(&r).contains("Only"));
    assert!(st.store.attachments(id).unwrap().is_empty());
    let r = send("scan.pdf", b"%PDF-1.4 hello").await.unwrap();
    assert!(loc(&r).contains("Attached"));
    let a = &st.store.attachments(id).unwrap()[0];
    let res = get_resp(&app, &format!("/attachments/{}", a.id)).await;
    assert_eq!(
        res.headers().get("content-type").unwrap(),
        "application/pdf"
    );
    assert!(res
        .headers()
        .get("content-security-policy")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("sandbox"));
    post(&app, &format!("/items/{id}/delete"), "", "same-origin").await;
    assert_eq!(
        std::fs::read_dir(dir.path().join("files")).unwrap().count(),
        0
    );
}

#[tokio::test]
async fn locations_page_and_calendar() {
    let (app, st) = app();
    post(&app, "/locations", "name=Garage", "same-origin").await;
    let (s, body) = get(&app, "/locations").await;
    assert_eq!(s, StatusCode::OK);
    assert!(body.contains("Garage"));
    let id = common::item(&st.store, "Beans, canned");
    st.store
        .add_batch(id, 3.0, common::d(10), None, None)
        .unwrap();
    let r = get_resp(&app, "/calendar.ics").await;
    assert!(r
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("text/calendar"));
    let body =
        String::from_utf8(r.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(
        body.contains("SUMMARY:Expires: Beans\\, canned (3 pcs)"),
        "{body}"
    );
    assert!(body.contains("\r\n"));
}

#[tokio::test]
async fn home_shows_monthly_spend_and_item_shows_prices() {
    let (app, st) = app();
    post(
        &app,
        "/add",
        "name=Coffee&qty=2&price=9%2C00",
        "same-origin",
    )
    .await;
    let id = st.store.all_items().unwrap()[0].id;
    let (_, body) = get(&app, "/").await;
    assert!(body.contains("Spent this month: €9.00"), "home");
    let (_, body) = get(&app, &format!("/items/{id}")).await;
    assert!(body.contains("Prices paid"));
    assert!(body.contains("€4.50 per piece"));
}

#[tokio::test]
async fn same_name_without_barcode_adds_to_existing_item() {
    let (app, st) = app();
    post(&app, "/add", "name=Candles&qty=2", "same-origin").await;
    post(&app, "/add", "name=candles+&qty=3", "same-origin").await;
    let items = st.store.all_items().unwrap();
    assert_eq!((items.len(), items[0].total), (1, 5.0));
}

#[tokio::test]
async fn link_a_scanned_code_to_an_existing_item() {
    let (app, st) = app();
    post(&app, "/add", "name=Oat+milk+Barista&qty=1", "same-origin").await;
    let id = st.store.all_items().unwrap()[0].id;
    let (_, body) = get(&app, "/add?barcode=999&name=Barista+oat").await;
    assert!(body.contains("Same thing"));
    post(
        &app,
        "/barcodes/link",
        &format!("code=999&item_id={id}"),
        "same-origin",
    )
    .await;
    let r = get_resp(&app, "/scan/code?code=999").await;
    assert_eq!(loc(&r), format!("/items/{id}"));
    let (_, body) = get(&app, &format!("/items/{id}")).await;
    assert!(body.contains("999"));
}

/// A free-text entry has no item to stock.
#[tokio::test]
async fn shopping_add_low_and_into_stock() {
    let (app, st) = app();
    let id = common::item(&st.store, "Lentils");
    st.store.set_min_qty(id, 3.0).unwrap();
    st.store
        .add_batch(id, 1.0, common::d(90), None, None)
        .unwrap();
    post(&app, "/shopping/add-low", "", "same-origin").await;
    let list = st.store.shopping().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].qty, Some(2.0));
    post(&app, "/shopping/add-low", "", "same-origin").await;
    assert_eq!(st.store.shopping().unwrap().len(), 1, "no duplicates");

    let r = post(
        &app,
        &format!("/shopping/{}/stock", list[0].id),
        "",
        "same-origin",
    )
    .await;
    let to = loc(&r);
    assert!(to.starts_with(&format!("/items/{id}?qty=2")), "{to}");
    assert!(to.ends_with("#add"), "{to}");
    assert!(st.store.shopping().unwrap().is_empty());
    let (s, body) = get(&app, to.trim_end_matches("#add")).await;
    assert_eq!(s, StatusCode::OK);
    assert!(body.contains("value=\"2\""), "qty prefilled");

    let e = st.store.add_shopping("Candles", None, None).unwrap();
    let r = post(&app, &format!("/shopping/{e}/stock"), "", "same-origin").await;
    assert!(loc(&r).starts_with("/shopping"));
}

#[tokio::test]
async fn shopping_page_and_dismiss_suggestion() {
    let (app, st) = app();
    let id = common::item(&st.store, "Oats");
    let (s, _) = get(&app, "/shopping").await;
    assert_eq!(s, StatusCode::OK);
    let r = post(
        &app,
        &format!("/shopping/suggestions/{id}/dismiss"),
        "",
        "same-origin",
    )
    .await;
    assert_eq!(r.status(), StatusCode::SEE_OTHER);
    assert!(loc(&r).contains("Hidden"), "{}", loc(&r));
}

#[tokio::test]
async fn item_page_shows_learned_hints() {
    let (app, st) = app();
    let id = common::item(&st.store, "Pasta");
    for _ in 0..3 {
        st.store
            .add_batch(id, 4.0, common::d(300), None, None)
            .unwrap();
    }
    let (s, body) = get(&app, &format!("/items/{id}")).await;
    assert_eq!(s, StatusCode::OK);
    assert!(body.contains("value=\"4\""), "usual quantity prefilled");
}

async fn json(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: &str,
    token: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let r = app
        .clone()
        .oneshot(b.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let s = r.status();
    let bytes = r.into_body().collect().await.unwrap().to_bytes();
    (
        s,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn api_writes_round_trip() {
    let (app, st) = app();
    let (s, v) = json(
        &app,
        "POST",
        "/api/v1/items",
        r#"{"name":"Water","category":"water","unit":"L","barcode":"4001","min_qty":12,"image":"../x"}"#,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let id = v["id"].as_i64().unwrap();
    assert_eq!(st.store.item(id).unwrap().unwrap().image, None);

    let (s, _) = json(
        &app,
        "POST",
        "/api/v1/items",
        r#"{"name":"Dup","barcode":"4001"}"#,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, v) = json(&app, "POST", "/api/v1/items", r#"{"name":" "}"#, None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(v["error"].as_str().unwrap().contains("name"));

    let uri = format!("/api/v1/items/{id}/batches");
    let (s, _) = json(
        &app,
        "POST",
        &uri,
        r#"{"qty":6,"expires_on":"2030-01-01"}"#,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let (s, _) = json(&app, "POST", &uri, r#"{"qty":-1}"#, None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = json(
        &app,
        "POST",
        "/api/v1/items/999/batches",
        r#"{"qty":1}"#,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let (s, v) = json(
        &app,
        "POST",
        &format!("/api/v1/items/{id}/consume"),
        r#"{"qty":2}"#,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["used"], 2.0);
    assert_eq!(st.store.item(id).unwrap().unwrap().total, 4.0);

    let (s, v) = json(&app, "GET", "/api/v1/barcode/4001", "", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["item"]["name"], "Water");
    assert_eq!(
        json(&app, "GET", "/api/v1/barcode/9", "", None).await.0,
        StatusCode::NOT_FOUND
    );

    let (_, v) = json(&app, "GET", "/api/v1/summary", "", None).await;
    assert_eq!(v["items"], 1);
    assert_eq!(v["low"], 1);

    let (s, _) = json(
        &app,
        "POST",
        "/api/v1/shopping",
        &format!(r#"{{"item_id":{id},"qty":8}}"#),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let (s, _) = json(&app, "POST", "/api/v1/shopping", "{}", None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (_, v) = json(&app, "GET", "/api/v1/shopping", "", None).await;
    assert_eq!(v[0]["label"], "Water");
    assert_eq!(
        json(&app, "GET", "/api/v1/suggestions", "", None).await.0,
        StatusCode::OK
    );
}

/// The pages don't need the token.
#[tokio::test]
async fn api_token_is_enforced_when_set() {
    let cfg =
        vorrat::config::Config::from_lookup(|k| (k == "VORRAT_API_TOKEN").then(|| "s3cret".into()))
            .unwrap();
    let app = router(Arc::new(AppState::new(common::store(), cfg)));
    assert_eq!(
        json(&app, "GET", "/api/v1/items", "", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        json(&app, "GET", "/api/v1/items", "", Some("nope")).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        json(&app, "GET", "/api/v1/items", "", Some("s3cret"))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(get(&app, "/").await.0, StatusCode::OK);
}

#[tokio::test]
async fn api_writes_refuse_cross_site_browsers() {
    let (app, _) = app();
    let r = post(&app, "/api/v1/items", "name=X", "cross-site").await;
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn plan_presets_and_printable_checklist() {
    let (app, st) = app();
    let (_, body) = get(&app, "/plan").await;
    assert!(body.contains("72 hours"));
    assert!(body.contains("/plan/print"));
    post(&app, "/plan/settings", "days=3", "same-origin").await;
    assert_eq!(st.store.setting_int("days", 0), 3);

    let id = common::item(&st.store, "Crackers");
    st.store
        .add_batch(id, 2.0, common::d(2), None, None)
        .unwrap();
    st.store
        .add_batch(id, 1.0, common::d(-1), None, None)
        .unwrap();
    let (s, body) = get(&app, "/plan/print").await;
    assert_eq!(s, StatusCode::OK);
    assert!(body.contains("2 people, 3 days"), "{body}");
    assert!(body.contains("Torch"));
    assert!(body.contains("Use first"));
    assert!(body.contains("Crackers"));
    assert!(!body.contains("<script>"), "no inline script");
}

#[tokio::test]
async fn export_and_import_through_the_ui() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_str().unwrap().to_string();
    let cfg = vorrat::config::Config::from_lookup(|k| match k {
        "VORRAT_DATA_DIR" => Some(d.clone()),
        "VORRAT_LOOKUP" => Some("off".into()),
        _ => None,
    })
    .unwrap();
    let st = Arc::new(AppState::new(common::store(), cfg));
    let app = router(st.clone());
    let (_, body) = get(&app, "/settings").await;
    assert!(body.contains("/export/stock.csv"));

    let body = "--X\r\nContent-Disposition: form-data; name=\"file\"; filename=\"s.csv\"\r\nContent-Type: text/csv\r\n\r\nname;qty;unit\nRice;2;kg\n\r\n--X--\r\n";
    let r = app
        .clone()
        .oneshot(
            Request::post("/import")
                .header("content-type", "multipart/form-data; boundary=X")
                .header("sec-fetch-site", "same-origin")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(loc(&r).starts_with("/items?msg=Imported"), "{}", loc(&r));
    assert_eq!(st.store.all_items().unwrap()[0].total, 2.0);

    let r = get_resp(&app, "/export/stock.csv").await;
    assert!(r.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .contains("attachment"));
    let (_, csv) = get(&app, "/export/stock.csv").await;
    assert!(csv.contains("Rice,,food"), "{csv}");
    let (s, json) = get(&app, "/export/vorrat.json").await;
    assert_eq!(s, StatusCode::OK);
    assert!(json.contains("\"Rice\""));
    let r = get_resp(&app, "/export/vorrat.db").await;
    assert_eq!(r.status(), StatusCode::OK);
    let bytes = r.into_body().collect().await.unwrap().to_bytes();
    assert!(bytes.starts_with(b"SQLite format 3\0"));
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        0,
        "temp file removed"
    );
}

async fn get_lang(app: &axum::Router, uri: &str, lang: &str) -> String {
    let r = app
        .clone()
        .oneshot(
            Request::get(uri)
                .header("accept-language", lang)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    String::from_utf8(r.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap()
}

#[tokio::test]
async fn german_from_the_browser_or_the_setting() {
    let (app, st) = app();
    let body = get_lang(&app, "/", "de-DE,de;q=0.9").await;
    assert!(body.contains("<html lang=\"de\">"));
    assert!(body.contains("Bestand") && body.contains("Notfallvorsorge"));
    let body = get_lang(&app, "/plan", "de").await;
    assert!(body.contains("Trinkwasser"), "default targets translated");
    assert!(get_lang(&app, "/", "en-GB")
        .await
        .contains("Emergency readiness"));

    post(&app, "/settings/lang", "lang=de", "same-origin").await;
    assert_eq!(st.store.setting("lang").unwrap().as_deref(), Some("de"));
    assert!(get_lang(&app, "/settings", "en")
        .await
        .contains("Einstellungen"));
    post(&app, "/settings/lang", "lang=en", "same-origin").await;
    assert!(get_lang(&app, "/", "de")
        .await
        .contains("Emergency readiness"));
    let r = post(&app, "/settings/lang", "lang=xx", "same-origin").await;
    assert!(loc(&r).contains("Unknown"));
}

fn app_with(vars: &'static [(&'static str, &'static str)]) -> axum::Router {
    let cfg = vorrat::config::Config::from_lookup(|k| {
        vars.iter()
            .find(|(n, _)| *n == k)
            .map(|(_, v)| v.to_string())
    })
    .unwrap();
    router(Arc::new(AppState::new(common::store(), cfg)))
}

async fn req(app: &axum::Router, r: Request<Body>) -> axum::response::Response {
    app.clone().oneshot(r).await.unwrap()
}

/// Pages send you to the login, with the way back. Writes and the API without a
/// session or token are refused. The API token still works on its own. Wrong
/// password, then the right one. Sign out ends the session for real. A forged
/// cookie doesn't help.
#[tokio::test]
async fn sign_in_protects_everything_when_a_password_is_set() {
    let app = app_with(&[
        ("VORRAT_PASSWORD", "correct horse"),
        ("VORRAT_API_TOKEN", "tok"),
    ]);
    let r = get_resp(&app, "/items?q=rice").await;
    assert_eq!(r.status(), StatusCode::SEE_OTHER);
    assert_eq!(loc(&r), "/login?next=%2Fitems%3Fq%3Drice");
    assert_eq!(get(&app, "/login").await.0, StatusCode::OK);
    assert_eq!(get(&app, "/static/app.css").await.0, StatusCode::OK);
    assert_eq!(
        get_resp(&app, "/export/vorrat.db").await.status(),
        StatusCode::SEE_OTHER
    );
    let r = post(&app, "/shopping", "label=X", "same-origin").await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(get(&app, "/api/v1/items").await.0, StatusCode::UNAUTHORIZED);
    let r = req(
        &app,
        Request::get("/api/v1/items")
            .header("authorization", "Bearer tok")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK);

    let r = post(&app, "/login", "password=nope&next=%2Fplan", "same-origin").await;
    assert!(loc(&r).contains("Wrong"), "{}", loc(&r));
    assert!(r.headers().get("set-cookie").is_none());
    let r = post(
        &app,
        "/login",
        "password=correct+horse&next=%2Fplan",
        "same-origin",
    )
    .await;
    assert_eq!(loc(&r), "/plan");
    let cookie = r.headers()["set-cookie"].to_str().unwrap().to_string();
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Lax"));
    let pair = cookie.split(';').next().unwrap().to_string();

    let with = |uri: &str| {
        Request::get(uri)
            .header("cookie", format!("other=1; {pair}"))
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(req(&app, with("/")).await.status(), StatusCode::OK);
    let body = req(&app, with("/settings"))
        .await
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    assert!(String::from_utf8_lossy(&body).contains("/logout"));

    let r = req(
        &app,
        Request::post("/logout")
            .header("cookie", pair.clone())
            .header("sec-fetch-site", "same-origin")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(loc(&r), "/login");
    assert_eq!(req(&app, with("/")).await.status(), StatusCode::SEE_OTHER);
    let r = req(
        &app,
        Request::get("/")
            .header("cookie", "vorrat_session=0000")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(r.status(), StatusCode::SEE_OTHER);
}

/// Cross-site login attempts are refused outright.
#[tokio::test]
async fn login_cannot_redirect_elsewhere_and_locks_out_guessing() {
    let app = app_with(&[("VORRAT_PASSWORD", "pw")]);
    let r = post(
        &app,
        "/login",
        "password=pw&next=%2F%2Fevil.example",
        "same-origin",
    )
    .await;
    assert_eq!(loc(&r), "/");
    for _ in 0..5 {
        post(&app, "/login", "password=x", "same-origin").await;
    }
    let r = post(&app, "/login", "password=pw", "same-origin").await;
    assert!(
        loc(&r).contains("Too+many") || loc(&r).contains("Too%20many"),
        "{}",
        loc(&r)
    );
    assert_eq!(
        post(&app, "/login", "password=pw", "cross-site")
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn without_a_password_there_is_no_login() {
    let (app, _) = app();
    assert_eq!(loc(&get_resp(&app, "/login").await), "/");
    assert_eq!(get(&app, "/").await.0, StatusCode::OK);
}

/// Normal use still works. Outside demo mode, no banner.
#[tokio::test]
async fn demo_mode_has_sample_data_and_blocks_server_changes() {
    let cfg =
        vorrat::config::Config::from_lookup(|k| (k == "VORRAT_DEMO").then(|| "on".into())).unwrap();
    let st = Arc::new(AppState::new(common::store(), cfg));
    vorrat::demo::reset(&st.store, common::today()).unwrap();
    let n = st.store.all_items().unwrap().len();
    assert!(n >= 10);
    vorrat::demo::reset(&st.store, common::today()).unwrap();
    assert_eq!(st.store.all_items().unwrap().len(), n, "reset, not doubled");
    let targets = st.store.targets().unwrap().len();
    assert!(targets > 0, "plan kept");

    let app = router(st.clone());
    let (_, body) = get(&app, "/").await;
    assert!(body.contains("demo-note"));
    assert!(body.contains("Yoghurt"), "something expiring on Home");
    let (_, body) = get(&app, "/plan/power-cut").await;
    assert!(body.contains("Frozen peas"));
    for p in [
        "/import",
        "/settings/backup",
        "/settings/lang",
        "/items/1/attachments",
    ] {
        let r = post(&app, p, "lang=de", "same-origin").await;
        assert!(loc(&r).contains("demo"), "{p}: {}", loc(&r));
    }
    let r = post(&app, "/shopping", "label=Candles", "same-origin").await;
    assert_eq!(r.status(), StatusCode::SEE_OTHER);
    let (app2, _) = self::app();
    assert!(!get(&app2, "/").await.1.contains("demo-note"));
}

#[tokio::test]
async fn french_and_dutch_pages() {
    let (app, _) = app();
    let fr = get_lang(&app, "/settings", "fr-FR,fr;q=0.9").await;
    assert!(fr.contains("<html lang=\"fr\">") && fr.contains("Réglages"));
    assert!(
        fr.contains("Nederlands"),
        "language list offers every language"
    );
    let nl = get_lang(&app, "/plan", "nl-NL").await;
    assert!(nl.contains("Noodplan") && nl.contains("Drinkwater"));
}

/// The plan page shows the household and the new scaling modes. Cross-site
/// writes are refused like everywhere else.
#[tokio::test]
async fn household_page_adds_edits_and_removes_people_and_pets() {
    let (app, st) = app();
    let (s, body) = get(&app, "/household").await;
    assert_eq!(s, StatusCode::OK);
    assert!(
        body.contains("Adult 1") && body.contains("Adult 2"),
        "{body}"
    );

    let r = post(
        &app,
        "/household",
        "kind=person&type=child&name=Mia+%3Cb%3E&need=vegetarian&need=allergies",
        "same-origin",
    )
    .await;
    assert!(r.headers()["location"].to_str().unwrap().contains("Added"));
    post(&app, "/household", "kind=pet&type=dog&name=", "same-origin").await;
    assert_eq!(st.store.household_counts().unwrap(), (3, 1));
    let (_, body) = get(&app, "/household").await;
    assert!(
        body.contains("Mia &#60;b&#62;") || body.contains("Mia &lt;b&gt;"),
        "names are escaped"
    );
    assert!(!body.contains("Mia <b>"));
    assert!(body.contains("vegetarian, allergies"));
    assert!(body.contains("Dog 1"));
    assert!(body.contains("No public body gives amounts for pets"));

    let (_, body) = get(&app, "/plan").await;
    assert!(body.contains("3 people"), "{body}");
    assert!(body.contains("1 pet"));
    assert!(body.contains("per pet per day"));

    let mia = st
        .store
        .members()
        .unwrap()
        .into_iter()
        .find(|m| m.name.as_deref() == Some("Mia <b>"))
        .unwrap();
    post(
        &app,
        &format!("/household/{}", mia.id),
        "type=teen&name=Mia",
        "same-origin",
    )
    .await;
    let m = st.store.members().unwrap();
    let mia = m.iter().find(|m| m.id == mia.id).unwrap();
    assert_eq!((mia.r#type.as_str(), mia.needs.len()), ("teen", 0));

    let r = post(&app, "/household", "kind=person&type=wizard", "same-origin").await;
    assert!(r.headers()["location"].to_str().unwrap().contains("Not"));

    post(
        &app,
        &format!("/household/{}", mia.id),
        "delete=1",
        "same-origin",
    )
    .await;
    assert_eq!(st.store.household_counts().unwrap(), (2, 1));

    let r = post(&app, "/household", "kind=pet&type=cat", "cross-site").await;
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    assert_eq!(st.store.household_counts().unwrap(), (2, 1));
}

#[tokio::test]
async fn household_page_in_german() {
    let (app, st) = app();
    st.store.add_member("pet", "cat", "", &[]).unwrap();
    let body = get_lang(&app, "/household", "de").await;
    assert!(body.contains("Haushalt"));
    assert!(body.contains("Erwachsene(r) 1"), "{body}");
    assert!(body.contains("Katze 1"));
}

/// The item page carries the dose form, to change or remove it. Bad input and
/// cross-site posts add nothing.
#[tokio::test]
async fn medicine_page_adds_a_medicine_and_its_dose() {
    let (app, st) = app();
    let (s, body) = get(&app, "/essentials").await;
    assert_eq!(s, StatusCode::OK);
    assert!(body.contains("never suggests a medicine or a dose"));
    assert!(body.contains("No medicines yet"));
    let who = st.store.members().unwrap()[0].id;

    let r = post(
        &app,
        "/essentials/medicine",
        &format!("name=Pill+%3Cx%3E&member_id={who}&form=tablet&strength=5+mg&unit=pcs&per_day=2&renew_on="),
        "same-origin",
    )
    .await;
    let to = loc(&r);
    assert!(
        to.starts_with("/items/") && to.contains("Medicine+added")
            || to.contains("Medicine%20added"),
        "{to}"
    );
    let id: i64 = to["/items/".len()..]
        .split('?')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    st.store.add_batch(id, 28.0, None, None, None).unwrap();

    let (_, body) = get(&app, "/essentials").await;
    assert!(body.contains("Adult 1"));
    assert!(body.contains("Pill &#60;x&#62;") || body.contains("Pill &lt;x&gt;"));
    assert!(!body.contains("Pill <x>"));
    assert!(body.contains("lasts 14 days"), "{body}");
    assert!(body.contains("5 mg · tablet"));

    let (_, body) = get(&app, &format!("/items/{id}")).await;
    assert!(body.contains("Daily dose") && body.contains("Remove daily dose"));
    post(
        &app,
        &format!("/items/{id}/dose"),
        "per_day=4&form=&strength=&member_id=",
        "same-origin",
    )
    .await;
    assert_eq!(st.store.dose(id).unwrap().unwrap().per_day, 4.0);
    let r = post(
        &app,
        &format!("/items/{id}/dose"),
        "per_day=0",
        "same-origin",
    )
    .await;
    assert!(loc(&r).contains("Not"));
    post(
        &app,
        &format!("/items/{id}/dose"),
        "delete=1",
        "same-origin",
    )
    .await;
    assert!(st.store.dose(id).unwrap().is_none());
    let (_, body) = get(&app, "/essentials").await;
    assert!(
        body.contains("without a daily dose"),
        "still in the medicine category"
    );

    let n = st.store.all_items().unwrap().len();
    let r = post(
        &app,
        "/essentials/medicine",
        "name=X&per_day=1&form=potion",
        "same-origin",
    )
    .await;
    assert!(loc(&r).starts_with("/essentials?msg=Not"));
    let r = post(
        &app,
        "/essentials/medicine",
        "name=X&per_day=1",
        "cross-site",
    )
    .await;
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    assert_eq!(st.store.all_items().unwrap().len(), n);
}

/// An item the household already has can be linked.
#[tokio::test]
async fn checklists_add_link_and_shop() {
    let (app, st) = app();
    let (_, body) = get(&app, "/essentials").await;
    assert!(
        body.contains("First-aid kit") && body.contains("0 / 11"),
        "{body}"
    );
    assert!(body.contains("Battery or wind-up radio"));
    assert!(body.contains("Named by: BE, HR"));

    let r = post(
        &app,
        "/essentials/check/es-radio",
        "action=add",
        "same-origin",
    )
    .await;
    let to = loc(&r);
    let id: i64 = to["/items/".len()..]
        .split('?')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let it = st.store.item(id).unwrap().unwrap();
    assert_eq!(
        (it.preset.as_deref(), it.category.as_deref()),
        (Some("es-radio"), Some("radio"))
    );
    let (_, body) = get(&app, &format!("/items/{id}")).await;
    assert!(body.contains("Counts for the checklist entry"));
    st.store.add_batch(id, 1.0, None, None, None).unwrap();

    let t = st
        .store
        .add_item(vorrat::db::NewItem {
            name: "Old thermometer".into(),
            category: Some("first aid".into()),
            ..Default::default()
        })
        .unwrap();
    let (_, body) = get(&app, "/essentials").await;
    assert!(body.contains("Old thermometer"));
    post(
        &app,
        "/essentials/check/fa-thermometer",
        &format!("action=link&item_id={t}"),
        "same-origin",
    )
    .await;
    assert_eq!(
        st.store.item(t).unwrap().unwrap().preset.as_deref(),
        Some("fa-thermometer")
    );
    post(&app, &format!("/items/{t}/preset"), "", "same-origin").await;
    assert!(st.store.item(t).unwrap().unwrap().preset.is_none());

    post(
        &app,
        "/essentials/check/es-cash",
        "action=shop",
        "same-origin",
    )
    .await;
    assert!(st
        .store
        .shopping()
        .unwrap()
        .iter()
        .any(|e| e.label == "Cash in small notes"));

    assert_eq!(
        post(&app, "/essentials/check/nope", "action=add", "same-origin")
            .await
            .status(),
        StatusCode::NOT_FOUND
    );

    let (_, body) = get(&app, "/plan").await;
    assert!(body.contains("Medicine and essentials"));
    assert!(body.contains("essentials 1/15"), "{body}");
}

#[tokio::test]
async fn medicine_page_in_german() {
    let (app, _) = app();
    let body = get_lang(&app, "/essentials", "de").await;
    assert!(body.contains("Medizin und Notwendiges"));
    assert!(body.contains("Verbandkasten"));
    assert!(body.contains("Batterie- oder Kurbelradio"));
}

/// Only suggestions can be hidden; checklist entries can't.
#[tokio::test]
async fn plan_suggests_from_the_household() {
    let (app, st) = app();
    let (_, body) = get(&app, "/plan").await;
    assert!(!body.contains("Suggested for your household"));
    st.store.add_member("pet", "cat", "Mimi <i>", &[]).unwrap();
    let (_, body) = get(&app, "/plan").await;
    assert!(body.contains("Suggested for your household"));
    assert!(body.contains("Pet food"));
    assert!(
        body.contains("for Mimi &#60;i&#62;") || body.contains("for Mimi &lt;i&gt;"),
        "{body}"
    );
    assert!(!body.contains("Lead<"), "a lead is for dogs");

    let r = post(
        &app,
        "/essentials/check/sg-pet-carrier",
        "action=dismiss",
        "same-origin",
    )
    .await;
    assert!(loc(&r).starts_with("/plan?msg=Hidden"));
    let r = post(
        &app,
        "/essentials/check/sg-pet-water",
        "action=shop",
        "same-origin",
    )
    .await;
    assert!(loc(&r).starts_with("/plan?"));
    let r = post(
        &app,
        "/essentials/check/sg-pet-food",
        "action=add",
        "same-origin",
    )
    .await;
    let id: i64 = loc(&r)["/items/".len()..]
        .split('?')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    st.store.add_batch(id, 2.0, None, None, None).unwrap();
    let (_, body) = get(&app, "/plan").await;
    assert!(!body.contains("Pet carrier or cage"));
    assert!(!body.contains(">Pet food<"));
    assert!(
        body.contains("Water for pets"),
        "on the list, not yet in stock"
    );

    assert_eq!(
        post(
            &app,
            "/essentials/check/es-radio",
            "action=dismiss",
            "same-origin"
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let body = get_lang(&app, "/plan", "de").await;
    assert!(body.contains("Vorschläge für deinen Haushalt") && body.contains("Wasser für Tiere"));
}

#[tokio::test]
async fn stock_filters_and_home_attention() {
    let (app, st) = app();
    let low = common::item(&st.store, "Lentils");
    st.store.set_min_qty(low, 3.0).unwrap();
    st.store
        .add_batch(low, 1.0, common::d(90), None, None)
        .unwrap();
    let fine = common::item(&st.store, "Rice");
    st.store
        .add_batch(fine, 5.0, common::d(300), None, None)
        .unwrap();

    let (_, all) = get(&app, "/items").await;
    assert!(all.contains("Lentils") && all.contains("Rice"));
    let (_, only_low) = get(&app, "/items?low=1").await;
    assert!(only_low.contains("Lentils") && !only_low.contains(">Rice<"));
    let (_, none) = get(&app, "/items?q=nothing-like-this").await;
    assert!(none.contains("Nothing matches."));

    let (_, home) = get(&app, "/").await;
    assert!(home.contains("Needs attention"));
    assert!(home.contains("Lentils"), "running low is on Home");
    assert!(home.contains("aria-current=\"page\""), "current tab marked");
}
