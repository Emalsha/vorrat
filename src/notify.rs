//! Push to the phone via ntfy (ntfy.sh or a self-hosted server), and the daily
//! digest job that uses it.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use chrono::NaiveDate;
use serde::Serialize;

use crate::config::Config;
use crate::db::Store;
use crate::rules;

pub type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// Sends (title, body); true on success. A function so tests can swap it.
pub type Notifier = Arc<dyn Fn(String, String) -> BoxFut<bool> + Send + Sync>;

/// The ntfy notifier, or `None` when no topic is configured.
pub fn ntfy(cfg: &Config) -> Option<Notifier> {
    if cfg.ntfy_topic.is_empty() {
        return None;
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(cfg.user_agent())
        .build()
        .ok()?;
    let url = format!("{}/{}", cfg.ntfy_url, cfg.ntfy_topic);
    let token = cfg.ntfy_token.clone();
    let click = (!cfg.public_url.is_empty()).then(|| format!("{}/", cfg.public_url));
    Some(Arc::new(move |title: String, body: String| {
        let mut req = client
            .post(&url)
            // ntfy accepts RFC 2047 for non-ASCII header values.
            .header("Title", header_safe(&title))
            .header("Tags", "canned_food")
            .body(body);
        if let Some(c) = &click {
            req = req.header("Click", c);
        }
        if !token.is_empty() {
            req = req.bearer_auth(&token);
        }
        Box::pin(async move {
            match req.send().await {
                Ok(r) if r.status().is_success() => true,
                Ok(r) => {
                    tracing::warn!("ntfy returned {}", r.status());
                    false
                }
                Err(e) => {
                    tracing::warn!("ntfy failed: {e}");
                    false
                }
            }
        })
    }))
}

/// HTTP header values must be ASCII; encode anything else as RFC 2047 base64.
fn header_safe(s: &str) -> String {
    if s.is_ascii() {
        return s.to_string();
    }
    use base64_lite::encode;
    format!("=?UTF-8?B?{}?=", encode(s.as_bytes()))
}

mod base64_lite {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    pub fn encode(b: &[u8]) -> String {
        let mut o = String::with_capacity(b.len().div_ceil(3) * 4);
        for c in b.chunks(3) {
            let n = (c[0] as u32) << 16
                | (*c.get(1).unwrap_or(&0) as u32) << 8
                | *c.get(2).unwrap_or(&0) as u32;
            o.push(T[(n >> 18) as usize & 63] as char);
            o.push(T[(n >> 12) as usize & 63] as char);
            o.push(if c.len() > 1 {
                T[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            o.push(if c.len() > 2 {
                T[n as usize & 63] as char
            } else {
                '='
            });
        }
        o
    }
}

#[derive(Debug, Serialize)]
pub struct DigestResult {
    pub sent: bool,
    pub reason: Option<String>,
    pub title: String,
    pub body: String,
}

/// Build today's digest and send it if there's anything new. `force` sends even
/// when there's nothing (the "test push" button). Only a delivered digest is
/// recorded, so a failed push is retried the next day.
pub async fn send_digest(
    store: &Store,
    remind: &rules::RemindDays,
    notifier: Option<&Notifier>,
    today: NaiveDate,
    force: bool,
) -> anyhow::Result<DigestResult> {
    let dg = rules::build_digest(store, today, remind)?;
    let (title, body) = if dg.is_empty() {
        (
            crate::i18n::tr("Vorrat: test push, all good"),
            crate::i18n::tr("Nothing expiring or running low right now."),
        )
    } else {
        (dg.title(), dg.body())
    };
    let Some(n) = notifier else {
        return Ok(DigestResult {
            sent: false,
            reason: Some("push is off (set VORRAT_NTFY_TOPIC)".into()),
            title,
            body,
        });
    };
    if dg.is_empty() && !force {
        return Ok(DigestResult {
            sent: false,
            reason: Some("nothing new".into()),
            title,
            body,
        });
    }
    let ok = n(title.clone(), body.clone()).await;
    if ok && !dg.is_empty() {
        rules::commit_digest(store, &dg)?;
    }
    Ok(DigestResult {
        sent: ok,
        reason: (!ok).then(|| "push failed, see the log".into()),
        title,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_ascii_titles_are_encoded() {
        assert_eq!(header_safe("Vorrat: 1 expired"), "Vorrat: 1 expired");
        assert_eq!(header_safe("Käse"), "=?UTF-8?B?S8Okc2U=?=");
    }
}
