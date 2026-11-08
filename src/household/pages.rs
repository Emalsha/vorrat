//! The household page.

use askama::Template;
use axum::extract::{Form, Path, Query, State};
use axum::response::{Html, Response};

use crate::essentials;
use crate::household::queries::{NEEDS, PERSON_TYPES, PET_TYPES};
use crate::i18n::tr;
use crate::web::respond::{done, render, Flash, Res};
use crate::web::view::filters;
use crate::web::St;

#[derive(Template)]
#[template(path = "household/household.html")]
struct HouseholdT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    people: Vec<MemberRow>,
    pets: Vec<MemberRow>,
    person_types: &'static [&'static str],
    pet_types: &'static [&'static str],
    needs: &'static [&'static str],
    pet_target: bool,
}

/// A person or pet as the household page shows it.
///
/// - `label`: the name, or the translated type with a number ("Adult 2").
/// - `needs`: every need with whether this person has it, for the checkboxes.
/// - `needs_text`: translated, for the summary line.
struct MemberRow {
    id: i64,
    label: String,
    name: String,
    r#type: String,
    needs: Vec<(&'static str, bool)>,
    needs_text: String,
}

pub(super) async fn household_page(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    let members = s.store.members()?;
    let labels = essentials::rules::member_labels(&members);
    let rows = |kind: &str| -> Vec<MemberRow> {
        members
            .iter()
            .filter(|m| m.kind == kind)
            .map(|m| MemberRow {
                id: m.id,
                label: labels[&m.id].clone(),
                name: m.name.clone().unwrap_or_default(),
                r#type: m.r#type.clone(),
                needs_text: m.needs.iter().map(|x| tr(x)).collect::<Vec<_>>().join(", "),
                needs: NEEDS
                    .iter()
                    .map(|n| (*n, m.needs.iter().any(|x| x == n)))
                    .collect(),
            })
            .collect()
    };
    let pet_target = s
        .store
        .targets()?
        .iter()
        .any(|t| t.mode.starts_with("per_pet"));
    render(HouseholdT {
        msg: f.msg,
        tab: "plan",
        undo: None,
        people: rows("person"),
        pets: rows("pet"),
        person_types: PERSON_TYPES,
        pet_types: PET_TYPES,
        needs: NEEDS,
        pet_target,
    })
}

/// Form fields as pairs: needs arrive as repeated `need=` checkboxes.
fn member_fields(f: &[(String, String)]) -> (String, String, String, Vec<String>, bool) {
    let get = |k: &str| {
        f.iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let needs = f
        .iter()
        .filter(|(n, _)| n == "need")
        .map(|(_, v)| v.clone())
        .collect();
    (
        get("kind"),
        get("type"),
        get("name"),
        needs,
        f.iter().any(|(n, _)| n == "delete"),
    )
}

pub(super) async fn add_member(State(s): St, Form(f): Form<Vec<(String, String)>>) -> Response {
    let (kind, r#type, name, needs, _) = member_fields(&f);
    done(
        "/household",
        s.store
            .add_member(&kind, &r#type, &name, &needs)
            .map(|_| "Added to the household".into()),
    )
}

pub(super) async fn edit_member(
    State(s): St,
    Path(id): Path<i64>,
    Form(f): Form<Vec<(String, String)>>,
) -> Response {
    let (_, r#type, name, needs, delete) = member_fields(&f);
    done(
        "/household",
        if delete {
            s.store
                .delete_member(id)
                .map(|_| "Removed from the household".into())
        } else {
            s.store
                .update_member(id, &r#type, &name, &needs)
                .map(|_| "Saved".into())
        },
    )
}
