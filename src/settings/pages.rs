//! The settings page, export and import.

use askama::Template;
use axum::extract::{Form, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;

use crate::web::respond::{back, download, render, Flash, Res};
use crate::web::view::filters;
use crate::web::St;
use crate::{db, jobs};

#[derive(Template)]
#[template(path = "settings/settings.html")]
struct SettingsT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    push: Option<String>,
    digest_time: String,
    backup: Option<String>,
    backup_time: String,
    lookup: bool,
    version: &'static str,
    lang: String,
    langs: Vec<(&'static str, &'static str)>,
    signed_in: bool,
}

/// The settings page. It never shows the ntfy topic: on ntfy.sh it works like a
/// password.
pub(super) async fn settings_page(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    let c = &s.cfg;
    render(SettingsT {
        msg: f.msg,
        tab: "settings",
        undo: None,
        push: (!c.ntfy_topic.is_empty()).then(|| c.ntfy_url.clone()),
        digest_time: format!("{:02}:{:02}", c.digest_time.0, c.digest_time.1),
        backup: c
            .backup_dir
            .as_ref()
            .map(|d| format!("{} (keeps {} days)", d.display(), c.backup_keep_days)),
        backup_time: format!("{:02}:{:02}", c.backup_time.0, c.backup_time.1),
        lookup: c.lookup_enabled,
        version: env!("CARGO_PKG_VERSION"),
        lang: s.lang_setting(),
        langs: crate::i18n::LANGS.iter().map(|l| (l.1, l.2)).collect(),
        signed_in: !s.cfg.password.is_empty(),
    })
}

#[derive(Deserialize)]
pub(super) struct LangForm {
    lang: String,
}

pub(super) async fn set_lang(State(s): St, Form(f): Form<LangForm>) -> Response {
    if f.lang != "auto" && !crate::i18n::LANGS.iter().any(|l| l.1 == f.lang) {
        return back("/settings", "Unknown language");
    }
    if let Err(e) = s.store.set_setting("lang", &f.lang) {
        return back("/settings", format!("Not saved: {e}"));
    }
    if let Ok(mut l) = s.lang.write() {
        *l = f.lang;
    }
    Redirect::to("/settings").into_response()
}

pub(super) async fn test_push(State(s): St) -> Response {
    match jobs::notify::send_digest(
        &s.store,
        &s.cfg.remind_days,
        s.notifier.as_ref(),
        db::today(),
        true,
    )
    .await
    {
        Ok(r) if r.sent => back("/settings", format!("Sent: {}", r.title)),
        Ok(r) => back(
            "/settings",
            format!("Not sent: {}", r.reason.unwrap_or_default()),
        ),
        Err(e) => back("/settings", format!("Not sent: {e}")),
    }
}

pub(super) async fn backup_now(State(s): St) -> Response {
    let Some(dir) = s.cfg.backup_dir.clone() else {
        return back("/settings", "Backup is off (set VORRAT_BACKUP_DIR)");
    };
    let st = s.clone();
    let r = tokio::task::spawn_blocking(move || {
        jobs::backup::run_backup(
            &st.store,
            &st.cfg.files_dir(),
            &dir,
            st.cfg.backup_keep_days,
            db::today(),
        )
    })
    .await;
    match r {
        Ok(Ok(p)) => back(
            "/settings",
            format!(
                "Backup written: {}",
                p.file_name().unwrap_or_default().to_string_lossy()
            ),
        ),
        Ok(Err(e)) => back("/settings", format!("Backup failed: {e}")),
        Err(e) => back("/settings", format!("Backup failed: {e}")),
    }
}

pub(super) async fn export_json(State(s): St) -> Res<Response> {
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let v = crate::transfer::export_json(&s.store, &now)?;
    Ok(download(
        "application/json",
        &format!("vorrat-{}.json", db::today()),
        serde_json::to_vec_pretty(&v)?,
    ))
}

/// The CSV export, with a BOM so Excel reads it as UTF-8 (umlauts).
pub(super) async fn export_csv(State(s): St) -> Res<Response> {
    let mut body = "\u{feff}".as_bytes().to_vec();
    body.extend(crate::transfer::export_csv(&s.store)?.into_bytes());
    Ok(download(
        "text/csv; charset=utf-8",
        &format!("vorrat-stock-{}.csv", db::today()),
        body,
    ))
}

/// The whole database as one SQLite file: the most complete copy, and it
/// opens in any Vorrat with the same schema as is.
pub(super) async fn export_db(State(s): St) -> Res<Response> {
    let tmp = s
        .cfg
        .data_dir
        .join(format!(".export-{}.db", crate::files::random_hex(8)));
    let st = s.clone();
    let path = tmp.clone();
    let bytes = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
        let r = st
            .store
            .backup_to(&path)
            .and_then(|_| Ok(std::fs::read(&path)?));
        let _ = std::fs::remove_file(&path);
        r
    })
    .await??;
    Ok(download(
        "application/vnd.sqlite3",
        &format!("vorrat-{}.db", db::today()),
        bytes,
    ))
}

pub(super) const MAX_IMPORT: usize = 5 * 1024 * 1024;

pub(super) async fn import_csv(State(s): St, mut mp: axum::extract::Multipart) -> Response {
    let mut text = None;
    loop {
        match mp.next_field().await {
            Ok(Some(f)) if f.name() == Some("file") => match f.bytes().await {
                Ok(b) => text = Some(b),
                Err(_) => return back("/settings", "Upload failed (max 5 MB)"),
            },
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(_) => return back("/settings", "Upload failed (max 5 MB)"),
        }
    }
    let Some(bytes) = text.filter(|b| !b.is_empty()) else {
        return back("/settings", "Choose a CSV file");
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return back("/settings", "Not a text file: save the CSV as UTF-8");
    };
    let text = text.to_string();
    let st = s.clone();
    let r =
        tokio::task::spawn_blocking(move || crate::transfer::import_csv(&st.store, &text)).await;
    match r {
        Ok(Ok(rep)) => back("/items", rep.summary()),
        Ok(Err(e)) => back("/settings", format!("Not imported: {e}")),
        Err(e) => back("/settings", format!("Not imported: {e}")),
    }
}
