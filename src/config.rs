//! Settings from environment variables. Nothing is baked into the binary.

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use crate::rules::RemindDays;

/// Settings for one run, read from `VORRAT_*` variables.
///
/// - `data_dir`: holds `vorrat.db` and `files/` (pictures, attachments).
/// - `listen`: where to listen. Loopback by default: put a TLS proxy (Caddy,
///   Tailscale Serve, …) in front for anything beyond this machine.
/// - `ntfy_url`: ntfy server; leave the topic empty to turn push off. On ntfy.sh the topic
///   works like a password: anyone who knows it can read the messages.
/// - `api_token`: when set, every `/api/` call needs `Authorization: Bearer <token>`.
/// - `password`: when set, the pages need this household password (sign-in).
/// - `public_url`: e.g. `https://vorrat.example.org`: tapping a push opens the app.
/// - `digest_time`: local time of the daily reminder digest and of the backup.
/// - `backup_dir`: `None` turns the nightly backup off.
/// - `lookup_enabled`: look up unknown barcodes in Open Food Facts (only the barcode is sent).
/// - `demo`: public demo: sample data, reset every night, no uploads or imports.
#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub listen: SocketAddr,
    pub currency: String,
    pub ntfy_url: String,
    pub ntfy_topic: String,
    pub ntfy_token: String,
    pub api_token: String,
    pub password: String,
    pub public_url: String,
    pub digest_time: (u32, u32),
    pub backup_time: (u32, u32),
    pub backup_dir: Option<PathBuf>,
    pub backup_keep_days: u32,
    pub lookup_enabled: bool,
    pub demo: bool,
    pub remind_days: RemindDays,
}

fn hm(raw: &str) -> Result<(u32, u32)> {
    let (h, m) = raw
        .trim()
        .split_once(':')
        .with_context(|| format!("{raw:?} is not HH:MM"))?;
    let (h, m): (u32, u32) = (h.parse()?, m.parse()?);
    if h > 23 || m > 59 {
        bail!("{raw:?} is not a time of day");
    }
    Ok((h, m))
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Self::from_lookup(|k| std::env::var(k).ok())
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let or = |k: &str, d: &str| get(k).unwrap_or_else(|| d.to_string());
        let listen = or("VORRAT_LISTEN", "127.0.0.1:8087");
        let backup = or("VORRAT_BACKUP_DIR", "");
        Ok(Self {
            data_dir: PathBuf::from(or("VORRAT_DATA_DIR", "./data")),
            listen: listen
                .parse()
                .with_context(|| format!("VORRAT_LISTEN={listen:?} is not host:port"))?,
            currency: or("VORRAT_CURRENCY", "EUR"),
            ntfy_url: or("VORRAT_NTFY_URL", "https://ntfy.sh")
                .trim_end_matches('/')
                .to_string(),
            ntfy_topic: or("VORRAT_NTFY_TOPIC", ""),
            ntfy_token: or("VORRAT_NTFY_TOKEN", ""),
            api_token: or("VORRAT_API_TOKEN", ""),
            password: or("VORRAT_PASSWORD", ""),
            public_url: or("VORRAT_PUBLIC_URL", "")
                .trim_end_matches('/')
                .to_string(),
            digest_time: hm(&or("VORRAT_DIGEST_TIME", "08:00")).context("VORRAT_DIGEST_TIME")?,
            backup_time: hm(&or("VORRAT_BACKUP_TIME", "03:30")).context("VORRAT_BACKUP_TIME")?,
            backup_dir: (!backup.is_empty()).then(|| PathBuf::from(backup)),
            backup_keep_days: or("VORRAT_BACKUP_KEEP_DAYS", "7")
                .parse()
                .context("VORRAT_BACKUP_KEEP_DAYS")?,
            lookup_enabled: !matches!(
                or("VORRAT_LOOKUP", "on").to_lowercase().as_str(),
                "off" | "0" | "false" | "no"
            ),
            demo: matches!(
                or("VORRAT_DEMO", "off").to_lowercase().as_str(),
                "on" | "1" | "true" | "yes"
            ),
            remind_days: RemindDays::default(),
        })
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("vorrat.db")
    }

    pub fn files_dir(&self) -> PathBuf {
        self.data_dir.join("files")
    }

    pub fn user_agent(&self) -> String {
        format!(
            "Vorrat/{} (self-hosted; https://github.com/Emalsha/vorrat)",
            env!("CARGO_PKG_VERSION")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_local_and_quiet() {
        let c = Config::from_lookup(|_| None).unwrap();
        assert!(c.listen.ip().is_loopback());
        assert_eq!(c.db_path(), PathBuf::from("./data/vorrat.db"));
        assert!(c.ntfy_topic.is_empty(), "no push unless configured");
        assert!(c.backup_dir.is_none());
        assert_eq!(c.digest_time, (8, 0));
    }

    #[test]
    fn bad_values_are_errors() {
        assert!(Config::from_lookup(|k| (k == "VORRAT_LISTEN").then(|| "nope".into())).is_err());
        assert!(
            Config::from_lookup(|k| (k == "VORRAT_DIGEST_TIME").then(|| "25:00".into())).is_err()
        );
    }

    #[test]
    fn lookup_switch() {
        assert!(
            !Config::from_lookup(|k| (k == "VORRAT_LOOKUP").then(|| "Off".into()))
                .unwrap()
                .lookup_enabled
        );
    }
}
