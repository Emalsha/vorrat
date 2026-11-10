//! The grab-bag page: each person's bag and what is still missing from it.
//! Which place is whose bag is kept in the settings (`grab_bag.<member id>`).

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, Response};

use crate::db;
use crate::essentials::rules::member_labels;
use crate::grab_bag::rules;
use crate::i18n::trf;
use crate::locations::queries::Location;
use crate::stock::rules::{annotate, fmt_qty};
use crate::web::respond::{done, render, Flash, Res};
use crate::web::view::filters;
use crate::web::St;

#[derive(Template)]
#[template(path = "grab_bag/grab_bag.html")]
struct GrabBagT {
    msg: Option<String>,
    tab: &'static str,
    undo: Option<String>,
    bags: Vec<Bag>,
}

/// One person's bag.
///
/// - `place`: the bag's place (id, name), when the person has one.
/// - `packed`: (entries ticked, entries).
struct Bag {
    member: i64,
    name: String,
    place: Option<(i64, String)>,
    lines: Vec<BagLine>,
    packed: (usize, usize),
}

/// One checklist entry as the page shows it.
///
/// - `items`: what is in the bag for it, with quantity and expiry date.
struct BagLine {
    label: &'static str,
    ok: bool,
    expired: bool,
    items: Vec<String>,
    named_by: String,
}

fn setting_key(member: i64) -> String {
    format!("grab_bag.{member}")
}

/// The person's bag, if its place still exists.
fn bag_place(s: &crate::web::AppState, member: i64, locs: &[Location]) -> Option<Location> {
    let id: i64 = s.store.setting(&setting_key(member)).ok()??.parse().ok()?;
    locs.iter().find(|l| l.id == id).cloned()
}

pub(super) async fn grab_bag_page(State(s): St, Query(f): Query<Flash>) -> Res<Html<String>> {
    let members = s.store.members()?;
    let labels = member_labels(&members);
    let locs = s.store.locations()?;
    let batches = annotate(s.store.batches(None)?, db::today());
    let bags = members
        .iter()
        .filter(|m| m.kind == "person")
        .map(|m| {
            let place = bag_place(&s, m.id, &locs);
            let lines: Vec<BagLine> = match &place {
                Some(p) => rules::check(&rules::inside(p.id, &locs), &batches)
                    .iter()
                    .map(|l| BagLine {
                        label: l.entry.label,
                        ok: l.ok(),
                        expired: l.expired(),
                        items: l
                            .batches
                            .iter()
                            .map(|b| {
                                let mut t = format!(
                                    "{} ({} {})",
                                    b.batch.item_name,
                                    fmt_qty(b.batch.qty),
                                    crate::i18n::tr(&b.batch.unit)
                                );
                                if let Some(e) = &b.batch.expires_on {
                                    t += &format!(" · {e}");
                                }
                                t
                            })
                            .collect(),
                        named_by: l.entry.named_by.join(", "),
                    })
                    .collect(),
                None => Vec::new(),
            };
            Bag {
                member: m.id,
                name: labels[&m.id].clone(),
                packed: (lines.iter().filter(|l| l.ok).count(), lines.len()),
                place: place.map(|p| (p.id, p.path)),
                lines,
            }
        })
        .collect();
    render(GrabBagT {
        msg: f.msg,
        tab: "plan",
        undo: None,
        bags,
    })
}

/// Makes a place for the person's bag, named after them, and remembers it.
pub(super) async fn make_bag(State(s): St, Path(member): Path<i64>) -> Response {
    done(
        "/grab-bag",
        (|| {
            let members = s.store.members()?;
            let m = members
                .iter()
                .find(|m| m.id == member && m.kind == "person")
                .ok_or_else(|| anyhow::anyhow!("no such person"))?;
            if bag_place(&s, m.id, &s.store.locations()?).is_some() {
                return Ok("Saved".into());
            }
            let name = trf("Grab bag: {0}", &[&member_labels(&members)[&m.id]]);
            let id = s.store.add_location(&name, None)?;
            s.store.set_setting(&setting_key(m.id), &id.to_string())?;
            Ok("Saved".into())
        })(),
    )
}
