//! Two daily jobs inside the app: the reminder digest and the backup.

use std::sync::Arc;
use std::time::Duration;

use chrono::{Local, NaiveDateTime, NaiveTime};

use crate::{backup, db, notify, web::AppState};

/// Next time `hm` (local) strictly after `now`.
pub fn next_run(now: NaiveDateTime, hm: (u32, u32)) -> NaiveDateTime {
    let t = NaiveTime::from_hms_opt(hm.0, hm.1, 0).expect("validated in config");
    let today = now.date().and_time(t);
    if today > now {
        today
    } else {
        today + chrono::Duration::days(1)
    }
}

/// Runs forever; spawn it on the runtime. The digest uses the saved language,
/// since there is no browser to ask. It wakes at least every 5 minutes, so
/// clock changes and sleep are handled.
pub async fn run(state: Arc<AppState>) {
    let cfg = &state.cfg;
    let mut next_digest = next_run(Local::now().naive_local(), cfg.digest_time);
    let mut next_backup = cfg
        .backup_dir
        .as_ref()
        .map(|_| next_run(Local::now().naive_local(), cfg.backup_time));
    tracing::info!("next digest at {next_digest}; next backup at {next_backup:?}");
    loop {
        let now = Local::now().naive_local();
        if now >= next_digest {
            if cfg.demo {
                if let Err(e) = crate::demo::reset(&state.store, db::today()) {
                    tracing::error!("demo reset failed: {e:#}");
                }
            }
            let lang = crate::i18n::Lang::pick(&state.lang_setting(), None);
            match crate::i18n::LANG
                .scope(
                    lang,
                    notify::send_digest(
                        &state.store,
                        &cfg.remind_days,
                        state.notifier.as_ref(),
                        db::today(),
                        false,
                    ),
                )
                .await
            {
                Ok(r) => tracing::info!(sent = r.sent, reason = ?r.reason, "digest: {}", r.title),
                Err(e) => tracing::error!("digest failed: {e:#}"),
            }
            next_digest = next_run(Local::now().naive_local(), cfg.digest_time);
        }
        if let (Some(when), Some(dir)) = (next_backup, cfg.backup_dir.clone()) {
            if now >= when {
                let st = state.clone();
                let files = cfg.files_dir();
                let keep = cfg.backup_keep_days;
                let r = tokio::task::spawn_blocking(move || {
                    backup::run_backup(&st.store, &files, &dir, keep, db::today())
                })
                .await;
                match r {
                    Ok(Ok(p)) => tracing::info!("backup ok: {}", p.display()),
                    Ok(Err(e)) => tracing::error!("backup failed: {e:#}"),
                    Err(e) => tracing::error!("backup task panicked: {e}"),
                }
                next_backup = Some(next_run(Local::now().naive_local(), cfg.backup_time));
            }
        }
        let soonest = next_backup.map_or(next_digest, |b| b.min(next_digest));
        let wait = (soonest - Local::now().naive_local())
            .num_seconds()
            .clamp(1, 300) as u64;
        tokio::time::sleep(Duration::from_secs(wait)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 7)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    #[test]
    fn next_run_today_or_tomorrow() {
        assert_eq!(next_run(at(7, 0), (8, 0)), at(8, 0));
        assert_eq!(
            next_run(at(8, 0), (8, 0)),
            at(8, 0) + chrono::Duration::days(1)
        );
        assert_eq!(
            next_run(at(9, 0), (8, 0)),
            at(8, 0) + chrono::Duration::days(1)
        );
    }
}
