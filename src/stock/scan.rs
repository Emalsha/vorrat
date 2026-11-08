//! The barcode scanner page and linking barcodes to items.

use askama::Template;
use axum::extract::{Form, Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;

use crate::i18n::tr;
use crate::stock::rules::fmt_qty;
use crate::web::respond::{back, done, render, url_encode, Res};
use crate::web::view::filters;
use crate::web::St;

#[derive(Deserialize)]
pub(super) struct LinkForm {
    code: String,
    item_id: i64,
}

pub(super) async fn link_barcode(State(s): St, Form(f): Form<LinkForm>) -> Response {
    let to = format!("/items/{}", f.item_id);
    done(
        &to,
        s.store
            .link_barcode(&f.code, f.item_id)
            .map(|_| format!("Barcode {} linked", f.code.trim())),
    )
}

#[derive(Deserialize)]
pub(super) struct CodeForm {
    code: String,
}

pub(super) async fn unlink_barcode(
    State(s): St,
    Path(id): Path<i64>,
    Form(f): Form<CodeForm>,
) -> Response {
    done(
        &format!("/items/{id}"),
        s.store
            .unlink_barcode(&f.code)
            .map(|_| "Barcode removed".into()),
    )
}

#[derive(Template)]
#[template(path = "stock/scan.html")]
struct ScanT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    mode: &'static str,
}

#[derive(Deserialize)]
pub(super) struct ScanQ {
    mode: Option<String>,
    msg: Option<String>,
}

pub(super) async fn scan_page(Query(q): Query<ScanQ>) -> Res<Html<String>> {
    let mode = if q.mode.as_deref() == Some("use") {
        "use"
    } else {
        "add"
    };
    render(ScanT {
        msg: q.msg,
        tab: "add",
        undo: None,
        mode,
    })
}

#[derive(Deserialize)]
pub(super) struct CodeQ {
    code: String,
    mode: Option<String>,
}

/// After a scan (or typed code): known item → its page (or use one up);
/// unknown → Open Food Facts → prefilled add form.
pub(super) async fn scan_code(State(s): St, Query(q): Query<CodeQ>) -> Res<Response> {
    let code = q.code.trim();
    if code.is_empty() {
        return Ok(Redirect::to("/scan").into_response());
    }
    let use_up = q.mode.as_deref() == Some("use");
    if let Some(it) = s.store.item_by_barcode(code)? {
        let to = format!("/items/{}", it.id);
        if use_up {
            if it.total <= 0.0 {
                return Ok(back(&to, format!("{}: none in stock", it.name)));
            }
            s.store.consume(it.id, 1.0, None, false)?;
            return Ok(back(&to, format!("{} 1 × {}", tr("Used"), it.name)));
        }
        return Ok(Redirect::to(&to).into_response());
    }
    if use_up {
        return Ok(back(
            "/scan?mode=use",
            format!("{code} is not in Vorrat yet"),
        ));
    }
    let mut params: Vec<(&str, String)> = vec![("barcode", code.to_string())];
    let found = if s.cfg.lookup_enabled {
        crate::lookup::lookup(code, &s.cfg.files_dir(), &s.cfg.user_agent()).await
    } else {
        None
    };
    match found {
        Some(f) => {
            params.push(("name", f.name));
            params.push(("kind", f.kind));
            params.push(("source", f.source));
            for (k, v) in [
                ("brand", f.brand),
                ("category", f.category.clone()),
                ("size_unit", f.size_unit),
                ("image", f.image),
                ("nutriscore", f.nutriscore),
            ] {
                if let Some(v) = v {
                    params.push((k, v));
                }
            }
            if let Some(sz) = f.size {
                params.push(("size", fmt_qty(sz)));
            }
            if let Some(n) = f.pack {
                params.push(("qty", n.to_string()));
            }
            if let Some(g) =
                crate::smart::guess_location(&s.store, f.category.as_deref(), f.frozen)?
            {
                params.push(("location_id", g.id.to_string()));
                params.push((
                    "hint",
                    format!("{} {}: {}.", tr("Location"), g.path, g.reason),
                ));
            }
        }
        None => params.push(("msg", "Not found in Open Food Facts. Type the name.".into())),
    }
    let qs: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{k}={}", url_encode(v)))
        .collect();
    Ok(Redirect::to(&format!("/add?{}", qs.join("&"))).into_response())
}
