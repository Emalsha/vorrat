//! Optional sign-in: one household password (`VORRAT_PASSWORD`). Sessions are
//! random tokens kept in memory, so a restart signs everyone out and signing
//! out really ends the session.

pub mod pages;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::routing::{get, post};
use axum::Router;

use crate::web::AppState;

pub const COOKIE: &str = "vorrat_session";
pub const SESSION_DAYS: u64 = 30;
const MAX_FAILS: u32 = 5;
const LOCKOUT: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct Sessions {
    live: Mutex<HashMap<String, Instant>>,
    fails: Mutex<(u32, Option<Instant>)>,
}

impl Sessions {
    /// A new session token, or `None` if the OS has no randomness to give.
    pub fn create(&self) -> Option<String> {
        let mut b = [0u8; 32];
        getrandom::getrandom(&mut b).ok()?;
        let token: String = b.iter().map(|x| format!("{x:02x}")).collect();
        let mut live = self.live.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        live.retain(|_, exp| *exp > now);
        live.insert(
            token.clone(),
            now + Duration::from_secs(SESSION_DAYS * 86_400),
        );
        Some(token)
    }

    pub fn valid(&self, token: &str) -> bool {
        let live = self.live.lock().unwrap_or_else(|e| e.into_inner());
        live.get(token).is_some_and(|exp| *exp > Instant::now())
    }

    pub fn end(&self, token: &str) {
        self.live
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(token);
    }

    /// While locked out after too many wrong passwords, how long to wait.
    pub fn locked_for(&self) -> Option<Duration> {
        let f = self.fails.lock().unwrap_or_else(|e| e.into_inner());
        match *f {
            (n, Some(since)) if n >= MAX_FAILS => LOCKOUT.checked_sub(since.elapsed()),
            _ => None,
        }
    }

    pub fn failed(&self) {
        let mut f = self.fails.lock().unwrap_or_else(|e| e.into_inner());
        if f.1.is_some_and(|t| t.elapsed() > LOCKOUT) {
            *f = (0, None);
        }
        f.0 += 1;
        f.1 = Some(Instant::now());
    }

    pub fn succeeded(&self) {
        *self.fails.lock().unwrap_or_else(|e| e.into_inner()) = (0, None);
    }
}

/// Constant-time comparison, so a guess can't be timed byte by byte.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The session token from a `Cookie` header.
pub fn token_from(cookie_header: Option<&str>) -> Option<&str> {
    cookie_header?
        .split(';')
        .filter_map(|c| c.trim().split_once('='))
        .find(|(k, _)| *k == COOKIE)
        .map(|(_, v)| v)
}

/// Paths open without signing in: the login page and what it needs.
pub fn is_public(path: &str) -> bool {
    matches!(
        path,
        "/login"
            | "/healthz"
            | "/static/app.css"
            | "/static/app.js"
            | "/sw.js"
            | "/static/manifest.webmanifest"
    ) || path.starts_with("/static/icon-")
}

/// Only a local path, so the login form can't send people elsewhere.
pub fn safe_next(next: Option<&str>) -> String {
    match next {
        Some(n) if n.starts_with('/') && !n.starts_with("//") && !n.contains('\\') => n.to_string(),
        _ => "/".to_string(),
    }
}

/// The sign-in and sign-out pages.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/login", get(pages::login_page).post(pages::login))
        .route("/logout", post(pages::logout))
}
