//! The shopping list page.

use askama::Template;
use axum::extract::{Form, Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;

use crate::shopping::queries::ShoppingEntry;
use crate::stock::rules::fmt_qty;
use crate::web::respond::{back, done, render, url_encode, Flash, Res};
use crate::web::view::filters;
use crate::web::St;
use crate::{db, stock};

#[derive(Template)]
#[template(path = "shopping/shopping.html")]
struct ShoppingT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    entries: Vec<ShoppingEntry>,
    any_done: bool,
    suggestions: Vec<(i64, String, String)>,
    low_missing: usize,
}

pub(super) async fn shopping_page(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    let entries = s.store.shopping()?;
    let listed: Vec<i64> = entries
        .iter()
        .filter(|e| !e.done)
        .filter_map(|e| e.item_id)
        .collect();
    let low_missing = stock::rules::low_stock(&s.store)?
        .iter()
        .filter(|i| !listed.contains(&i.id))
        .count();
    let suggestions = crate::smart::shopping_suggestions(&s.store, db::today())?
        .into_iter()
        .map(|g| (g.item.id, g.item.name, g.reason))
        .collect();
    render(ShoppingT {
        msg: f.msg,
        tab: "shopping",
        undo: None,
        any_done: entries.iter().any(|e| e.done),
        entries,
        suggestions,
        low_missing,
    })
}

#[derive(Deserialize)]
pub(super) struct ShoppingForm {
    label: String,
    item_id: Option<i64>,
}

pub(super) async fn add_shopping(State(s): St, Form(f): Form<ShoppingForm>) -> Response {
    done(
        "/shopping",
        s.store
            .add_shopping(&f.label, f.item_id, None)
            .map(|_| "Added".into()),
    )
}

/// Everything running low that isn't on the list yet.
pub(super) async fn shopping_add_low(State(s): St) -> Response {
    done(
        "/shopping",
        (|| {
            let listed: Vec<i64> = s
                .store
                .shopping()?
                .into_iter()
                .filter(|e| !e.done)
                .filter_map(|e| e.item_id)
                .collect();
            let mut n = 0;
            for it in stock::rules::low_stock(&s.store)?
                .into_iter()
                .filter(|i| !listed.contains(&i.id))
            {
                s.store
                    .add_shopping(&it.name, Some(it.id), Some(it.min_qty - it.total))?;
                n += 1;
            }
            Ok(format!("{n} added"))
        })(),
    )
}

/// A bought entry → the item's add-stock form, prefilled; the entry goes away.
pub(super) async fn shopping_to_stock(State(s): St, Path(id): Path<i64>) -> Response {
    let r = (|| -> anyhow::Result<Option<(i64, String, f64)>> {
        let Some(e) = s.store.shopping()?.into_iter().find(|e| e.id == id) else {
            return Ok(None);
        };
        let Some(item) = e.item_id else {
            return Ok(None);
        };
        s.store.delete_shopping(id)?;
        let qty = e.qty.filter(|q| *q > 0.0).map(f64::ceil).unwrap_or(1.0);
        Ok(Some((item, e.item_name.unwrap_or(e.label), qty)))
    })();
    match r {
        Ok(Some((item, name, qty))) => Redirect::to(&format!(
            "/items/{item}?qty={}&msg={}#add",
            fmt_qty(qty),
            url_encode(&format!("Bought {name}: add the expiry date and save"))
        ))
        .into_response(),
        Ok(None) => back("/shopping", "Not linked to an item"),
        Err(e) => back("/shopping", format!("Not saved: {e}")),
    }
}

pub(super) async fn dismiss_suggestion(State(s): St, Path(id): Path<i64>) -> Response {
    done(
        "/shopping",
        crate::smart::dismiss_suggestion(&s.store, id, db::today())
            .map(|_| "Hidden for 14 days".into()),
    )
}

pub(super) async fn toggle_shopping(State(s): St, Path(id): Path<i64>) -> Response {
    match s.store.toggle_shopping(id) {
        Ok(()) => Redirect::to("/shopping").into_response(),
        Err(e) => back("/shopping", format!("Not saved: {e}")),
    }
}

pub(super) async fn clear_shopping(State(s): St) -> Response {
    done(
        "/shopping",
        s.store.clear_shopping_done().map(|_| "Cleared".into()),
    )
}
