//! Sign-in, sign-out and the middleware that asks for the household password.

use askama::Template;
use axum::extract::{Form, Query, Request, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;

use crate::auth;
use crate::auth::{COOKIE, SESSION_DAYS};
use crate::web::respond::{render, url_encode, Res};
use crate::web::view::filters;
use crate::web::St;

/// With `VORRAT_PASSWORD` set, every page needs a session. API calls with the
/// bearer token (`VORRAT_API_TOKEN`) don't; `api_auth` checks those.
pub(crate) async fn sign_in(State(s): St, req: Request, next: Next) -> Response {
    if s.cfg.password.is_empty() {
        return next.run(req).await;
    }
    let path = req.uri().path();
    let bearer = req.headers().contains_key(header::AUTHORIZATION);
    if auth::is_public(path) || (path.starts_with("/api/") && bearer && !s.cfg.api_token.is_empty())
    {
        return next.run(req).await;
    }
    let cookies = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok());
    if auth::token_from(cookies).is_some_and(|t| s.sessions.valid(t)) {
        return next.run(req).await;
    }
    if req.method() == Method::GET && !path.starts_with("/api/") {
        let here = req
            .uri()
            .path_and_query()
            .map(|p| p.as_str())
            .unwrap_or("/");
        return Redirect::to(&format!("/login?next={}", url_encode(here))).into_response();
    }
    (StatusCode::UNAUTHORIZED, "sign in first").into_response()
}

#[derive(Template)]
#[template(path = "auth/login.html")]
struct LoginT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    next: String,
}

#[derive(Deserialize)]
pub(super) struct LoginQ {
    next: Option<String>,
    msg: Option<String>,
}

pub(super) async fn login_page(State(s): St, Query(q): Query<LoginQ>) -> Res<Response> {
    if s.cfg.password.is_empty() {
        return Ok(Redirect::to("/").into_response());
    }
    Ok(render(LoginT {
        msg: q.msg,
        tab: "login",
        undo: None,
        next: auth::safe_next(q.next.as_deref()),
    })?
    .into_response())
}

#[derive(Deserialize)]
pub(super) struct LoginForm {
    password: String,
    next: Option<String>,
}

/// Checks the household password. A wrong one waits a little longer, to slow
/// down guessing.
pub(super) async fn login(State(s): St, Form(f): Form<LoginForm>) -> Response {
    let next = auth::safe_next(f.next.as_deref());
    let again = |msg: &str| {
        Redirect::to(&format!(
            "/login?next={}&msg={}",
            url_encode(&next),
            url_encode(msg)
        ))
        .into_response()
    };
    if s.cfg.password.is_empty() {
        return Redirect::to("/").into_response();
    }
    if s.sessions.locked_for().is_some() {
        return again("Too many attempts. Wait a minute.");
    }
    if !auth::ct_eq(f.password.as_bytes(), s.cfg.password.as_bytes()) {
        s.sessions.failed();
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        tracing::warn!("wrong password at sign-in");
        return again("Wrong password");
    }
    s.sessions.succeeded();
    let Some(token) = s.sessions.create() else {
        return again("Could not start a session");
    };
    let secure = if s.cfg.public_url.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };
    let cookie = format!(
        "{}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{secure}",
        COOKIE,
        SESSION_DAYS * 86_400
    );
    ([(header::SET_COOKIE, cookie)], Redirect::to(&next)).into_response()
}

pub(super) async fn logout(State(s): St, headers: HeaderMap) -> Response {
    let cookies = headers.get(header::COOKIE).and_then(|v| v.to_str().ok());
    if let Some(t) = auth::token_from(cookies) {
        s.sessions.end(t);
    }
    (
        [(
            header::SET_COOKIE,
            format!("{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0", COOKIE),
        )],
        Redirect::to("/login"),
    )
        .into_response()
}
