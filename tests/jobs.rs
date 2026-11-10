mod common;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use common::*;
use vorrat::jobs::digest::RemindDays;
use vorrat::jobs::notify::{send_digest, Notifier};

type Sent = Arc<Mutex<Vec<(String, String)>>>;

fn fake(ok: bool) -> (Notifier, Sent) {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let s2 = sent.clone();
    let n: Notifier = Arc::new(move |t, b| {
        s2.lock().unwrap().push((t, b));
        Box::pin(async move { ok })
    });
    (n, sent)
}

#[tokio::test]
async fn digest_is_sent_once_and_recorded() {
    let s = store();
    let i = item(&s, "Milk");
    s.add_batch(i, 1.0, d(3), None, None).unwrap();
    let (n, sent) = fake(true);
    let r = send_digest(&s, &RemindDays::default(), Some(&n), today(), false)
        .await
        .unwrap();
    assert!(r.sent);
    assert_eq!(sent.lock().unwrap()[0].0, "Vorrat: 1 expiring this week");
    let r = send_digest(&s, &RemindDays::default(), Some(&n), today(), false)
        .await
        .unwrap();
    assert!(!r.sent, "nothing new the second time");
    assert_eq!(sent.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn failed_push_is_retried_next_time() {
    let s = store();
    let i = item(&s, "Milk");
    s.add_batch(i, 1.0, d(3), None, None).unwrap();
    let (bad, _) = fake(false);
    assert!(
        !send_digest(&s, &RemindDays::default(), Some(&bad), today(), false)
            .await
            .unwrap()
            .sent
    );
    let (good, sent) = fake(true);
    assert!(
        send_digest(&s, &RemindDays::default(), Some(&good), today(), false)
            .await
            .unwrap()
            .sent
    );
    assert_eq!(sent.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn test_push_sends_even_when_nothing_is_due() {
    let s = store();
    let calls = Arc::new(AtomicUsize::new(0));
    let c2 = calls.clone();
    let n: Notifier = Arc::new(move |_, _| {
        c2.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { true })
    });
    assert!(
        send_digest(&s, &RemindDays::default(), Some(&n), today(), true)
            .await
            .unwrap()
            .sent
    );
    assert!(
        !send_digest(&s, &RemindDays::default(), Some(&n), today(), false)
            .await
            .unwrap()
            .sent
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn no_topic_means_no_push() {
    let s = store();
    let r = send_digest(&s, &RemindDays::default(), None, today(), true)
        .await
        .unwrap();
    assert!(!r.sent);
    assert!(r.reason.unwrap().contains("VORRAT_NTFY_TOPIC"));
}

#[test]
fn backup_writes_a_consistent_copy_and_prunes() {
    let dir = tempfile::tempdir().unwrap();
    let s = vorrat::db::Store::open(dir.path().join("vorrat.db")).unwrap();
    item(&s, "Rice");
    let files = dir.path().join("files");
    std::fs::create_dir_all(&files).unwrap();
    std::fs::write(files.join("img-1.jpg"), b"x").unwrap();
    let out = dir.path().join("backup");
    std::fs::create_dir_all(&out).unwrap();
    let old = out.join("vorrat-2000-01-01.db");
    std::fs::write(&old, b"old").unwrap();
    let f = std::fs::File::options().write(true).open(&old).unwrap();
    f.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(30 * 86_400))
        .unwrap();

    let p = vorrat::jobs::backup::run_backup(&s, &files, &out, 7, today()).unwrap();
    let copy = vorrat::db::Store::open(&p).unwrap();
    assert_eq!(copy.all_items().unwrap()[0].name, "Rice");
    assert!(out
        .join(format!(
            "vorrat-{}-files.tar.gz",
            today().format("%Y-%m-%d")
        ))
        .exists());
    assert!(!old.exists(), "copies older than keep_days are removed");
}

#[tokio::test]
async fn digest_in_german_when_set() {
    let s = common::store();
    let id = common::item(&s, "Milch");
    s.add_batch(id, 1.0, common::d(-1), None, None).unwrap();
    let dg = vorrat::jobs::digest::build_digest(&s, common::today(), &Default::default()).unwrap();
    let (title, body) = vorrat::i18n::LANG
        .scope(vorrat::i18n::Lang::De, async { (dg.title(), dg.body()) })
        .await;
    assert_eq!(title, "Vorrat: 1 abgelaufen");
    assert!(
        body.starts_with("Abgelaufen: Milch (1 Stk., vor 1 T.)"),
        "{body}"
    );
    assert!(
        dg.title().contains("1 expired"),
        "English outside a German request"
    );
}

/// The kit check is reminded once, about six months after the last check or
/// after Vorrat started counting, and again only after the next check.
#[test]
fn the_kit_check_is_reminded_twice_a_year() {
    use chrono::Days;
    use vorrat::jobs::digest::{build_digest, commit_digest};
    let s = common::store();
    let t = common::today();
    let dg = build_digest(&s, t, &Default::default()).unwrap();
    assert!(!dg.review, "a new household starts counting today");
    let later = t + Days::new(182);
    let dg = build_digest(&s, later, &Default::default()).unwrap();
    assert!(dg.review);
    assert!(dg.body().contains("Twice-yearly check"));
    commit_digest(&s, &dg).unwrap();
    assert!(
        !build_digest(&s, later, &Default::default()).unwrap().review,
        "once"
    );
    s.set_setting("reviewed_on", &later.to_string()).unwrap();
    assert!(
        !build_digest(&s, later + Days::new(181), &Default::default())
            .unwrap()
            .review
    );
    assert!(
        build_digest(&s, later + Days::new(182), &Default::default())
            .unwrap()
            .review
    );
}
