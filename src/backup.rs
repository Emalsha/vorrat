//! Nightly copy of the data to a backup folder (for restic, borg, a NAS, …).
//! The database copy uses SQLite's online backup API, so it is consistent while
//! the app runs. Each file is written under a temporary name and renamed, so a
//! backup tool never sees a half-written copy.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use chrono::NaiveDate;

use crate::db::Store;

/// Copies the database and the files folder into `backup_dir`, then removes
/// copies older than `keep_days` days.
pub fn run_backup(
    store: &Store,
    files_dir: &Path,
    backup_dir: &Path,
    keep_days: u32,
    today: NaiveDate,
) -> Result<PathBuf> {
    fs::create_dir_all(backup_dir).with_context(|| format!("creating {}", backup_dir.display()))?;
    let stamp = today.format("%Y-%m-%d");

    let db_out = backup_dir.join(format!("vorrat-{stamp}.db"));
    let tmp = backup_dir.join(format!(".vorrat-{stamp}.db.tmp"));
    let _ = fs::remove_file(&tmp);
    store.backup_to(&tmp)?;
    fs::rename(&tmp, &db_out)?;

    let files_out = backup_dir.join(format!("vorrat-{stamp}-files.tar.gz"));
    let ftmp = backup_dir.join(format!(".vorrat-{stamp}-files.tar.gz.tmp"));
    {
        let gz =
            flate2::write::GzEncoder::new(fs::File::create(&ftmp)?, flate2::Compression::default());
        let mut tar = tar::Builder::new(gz);
        tar.follow_symlinks(false);
        if files_dir.is_dir() {
            tar.append_dir_all("files", files_dir)?;
        }
        tar.into_inner()?.finish()?;
    }
    fs::rename(&ftmp, &files_out)?;

    let cutoff = SystemTime::now() - Duration::from_secs(u64::from(keep_days) * 86_400);
    for e in fs::read_dir(backup_dir)?.flatten() {
        let name = e.file_name();
        if !name.to_string_lossy().starts_with("vorrat-") {
            continue;
        }
        if e.metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|t| t < cutoff)
        {
            let _ = fs::remove_file(e.path());
        }
    }
    Ok(db_out)
}
