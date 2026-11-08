//! The locations page.

use askama::Template;
use axum::extract::{Form, Path, Query, State};
use axum::response::{Html, Response};
use serde::Deserialize;

use crate::web::form::opt_i64;
use crate::web::respond::{done, render, Flash, Res};
use crate::web::view::filters;
use crate::web::St;

#[derive(Template)]
#[template(path = "locations/locations.html")]
struct LocationsT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    locations: Vec<LocationRow>,
}

struct LocationRow {
    id: i64,
    name: String,
    path: String,
    count: i64,
    parent_id: Option<i64>,
}

pub(super) async fn locations_page(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    let counts = s.store.location_counts()?;
    let locations = s
        .store
        .locations()?
        .into_iter()
        .map(|l| LocationRow {
            count: counts.get(&l.id).copied().unwrap_or(0),
            id: l.id,
            name: l.name,
            path: l.path,
            parent_id: l.parent_id,
        })
        .collect();
    render(LocationsT {
        msg: f.msg,
        tab: "items",
        undo: None,
        locations,
    })
}

#[derive(Deserialize)]
pub(super) struct LocForm {
    name: String,
    parent_id: Option<String>,
    delete: Option<String>,
}

pub(super) async fn add_location(State(s): St, Form(f): Form<LocForm>) -> Response {
    done(
        "/locations",
        s.store
            .add_location(&f.name, opt_i64(&f.parent_id))
            .map(|_| "Added".into()),
    )
}

pub(super) async fn edit_location(
    State(s): St,
    Path(id): Path<i64>,
    Form(f): Form<LocForm>,
) -> Response {
    if f.delete.is_some() {
        return done(
            "/locations",
            s.store
                .delete_location(id)
                .map(|_| "Deleted (its stock is now without a location)".into()),
        );
    }
    done(
        "/locations",
        s.store
            .rename_location(id, &f.name, opt_i64(&f.parent_id))
            .map(|_| "Saved".into()),
    )
}
