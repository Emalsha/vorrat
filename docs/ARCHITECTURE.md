# Project structure

*Agreed and adopted November 2026. Read this before adding a new module or
folder.*

## The short answer

Rust has one structure everybody follows, and it is small: the Cargo package
layout (`src/main.rs`, `src/lib.rs`, `tests/`, `benches/`, `examples/`).
Above that there is no house style like Clean Architecture in .NET. Projects
pick from a few common patterns, described below.

For Vorrat we chose:

- **One crate**, as today. No workspace with several crates yet.
- **Vertical slices**: one folder per feature (stock, plan, shopping...), each
  holding that feature's pages, queries and rules.
- **A thin shared core** for what every feature needs: the database
  connection and schema, the web plumbing (router, security, errors), sign-in,
  config and translations.
- **The Clean Architecture dependency rule, kept simple**: the logic that
  decides things (expiry, coverage, hints) does not know about HTTP or HTML.
  No interfaces or repositories to get there.

## What the web app needs from its structure

These come from `AGENTS.md` and `docs/PLAN.md`. Any structure has to serve
them; the last group is what "easy to maintain as it grows" means here.

**Runtime**

- One binary and one SQLite file. Runs offline on a Raspberry Pi (64-bit ARM),
  today about 7 MB on disk and 13 MB of memory.
- Server-rendered HTML pages (Askama, compiled in) that work without
  JavaScript, plus a JSON API under `/api/v1` for a future mobile app and Home
  Assistant.
- Background jobs in the same process: daily digest, ntfy push, nightly
  backup, nightly demo reset.
- Only two optional outbound calls, Open Food Facts and the user's ntfy
  server. Both time out and nothing depends on them.

**Safety**

- Every write is a POST behind the same-origin check, redirects go through
  `safe_back`/`safe_next`, uploads are checked by their bytes, strict CSP.
- Optional household password, optional API token, demo mode that can't touch
  the server.
- The database format stays compatible: schema changes are versioned in one
  place.

**Maintenance**

- One person reviews everything, and most changes are written by agents. A
  change to one feature should touch one folder and be easy to review.
- The security rules live in one place, so a new page gets them without
  remembering to.
- Pure rules can be tested without a server or a database file.
- No framework, no front-end build step, few dependencies, and no
  abstraction for something that doesn't exist yet (rule 9).
- Adding a feature should mean adding a folder, not editing five large files.

## Why we moved

Until November 2026 Vorrat was one crate layered by technical role:
`src/web.rs` (3,100 lines: router, middleware, every page and the API),
`src/db.rs` (2,100 lines: schema and every query) and `src/rules.rs`. That
was the right shape for a first version, but a change to the shopping list
touched three large files far apart, and those files were where every pull
request collided.

## Common Rust structures, compared

| Pattern | What it looks like | Who uses it | Fit for Vorrat |
|---|---|---|---|
| **Single crate, layered** | `src/routes/`, `src/db/`, `src/domain/` | Most small and mid-size web apps; *Zero to Production in Rust* | What we have, in files instead of folders. Fine, but features get spread out |
| **Single crate, feature folders** (vertical slices) | `src/shopping/`, `src/plan/`, each with its own handlers and queries | Common in axum apps that grew past a few thousand lines | **Recommended.** Keeps a feature in one place, costs nothing at build or run time |
| **Workspace with several crates** | `crates/core`, `crates/db`, `crates/web`, one `Cargo.toml` each | Large projects such as rust-analyzer and Lemmy | Not yet. Enforces boundaries and speeds up rebuilds, but adds a `Cargo.toml` per crate and version bookkeeping. Worth it past roughly 30,000 lines or with a second binary |
| **Hexagonal / ports and adapters** | A trait per repository or service, adapters implement it, handlers get `Arc<dyn Trait>` | Blog posts and templates, few apps of Vorrat's size | No. With one database and one web front end, every trait has exactly one implementation. That is the abstraction rule 9 rules out |

### For a .NET developer

| .NET | Rust |
|---|---|
| Solution | Workspace |
| Project / assembly | Crate |
| Namespace and folder | Module (`mod`) and folder: they always match |
| `internal` | `pub(crate)` |
| Interface | Trait |
| DI container | No container: shared things go in `AppState`, which axum hands to each handler |
| Controller / MediatR handler | A plain `async fn` handler |
| Partial class | Several `impl Store` blocks in different files of the same crate |

The last line is what makes vertical slices cheap in Rust: every feature can
add its own queries to the one `Store` type in its own file, with no
repository interface in between.

## Layout

```text
src/
  main.rs              start-up, signals, server
  lib.rs               module list
  config.rs            VORRAT_* settings
  i18n.rs              tr, trf, Lang
  files.rs             safe generated file names

  db/                  shared: the store, knows no feature
    mod.rs             Store: one writer, pooled readers, settings, backup, export
    schema.rs          SCHEMA list, seed data and the migration loop (the only place)
  web/                 shared: web plumbing
    mod.rs             AppState, router assembly, security and language middleware, static files
    respond.rs         AppError, render, back/done/safe_back, flash messages, downloads
    form.rs            optional numbers and dates from form fields
    view.rs            Askama filters, date and money helpers
  auth/                household password, sessions, the sign-in pages

  home/                the home page
  stock/               items, batches, barcodes, attachments, calendar
    mod.rs             routes() for this feature
    pages.rs           handlers, form structs, template structs
    scan.rs            the scanner page and barcodes
    attachments.rs     pictures and documents on an item
    queries.rs         impl Store { items, batches, consume, undo, history ... }
    rules.rs           expiry status, low stock, quantities
  locations/           pages.rs, queries.rs
  plan/                coverage, targets, print view, power cut
  household/           people and pets
  country/             country presets from docs/guidance (days, drinking water)
  essentials/          medicine, doses, checklists
  shopping/            the list and dismissed suggestions
  settings/            language, test push, backup, export, import
  api/                 JSON under /api/v1, calls the same queries and rules

  jobs/                scheduler, notify (ntfy), backup, digest, what was notified
  smart.rs             learned hints
  lookup.rs            Open Food Facts
  transfer.rs          CSV and JSON export, CSV import
  demo.rs              sample household

templates/             one folder per feature, shared layout and partials at the top
  base.html  _icon.html  _thumb.html  _batch_row.html  _ready.html
  stock/item.html  plan/plan.html  essentials/_checklist.html ...
```

A feature folder has `mod.rs` with its `routes()` and some of `pages.rs`,
`queries.rs` and `rules.rs`. A feature with many screens may split its pages
by screen, as `stock/` does with `scan.rs` and `attachments.rs`.
`web/mod.rs` merges every feature's routes and wraps them all in the same
layers (API token, household password, language, security headers), so a new
page can't miss them. It is the one shared module that names the features.

The shared modules sit at the top of `src/` instead of in a `core/` folder:
`core` is also the name of Rust's built-in library, and `crate::db::Store`
reads better than `crate::core::db::Store`.

### Rules for the layout

1. **A feature depends on the shared modules, never the other way round.**
   `db/`, `auth/`, `config.rs`, `i18n.rs` and `files.rs` name no feature;
   `web/mod.rs` names them only to merge their routes.
2. **A feature uses another feature only through its `pub(crate)` functions**,
   never its tables. The plan reads stock through `stock::queries`, it doesn't
   write its own `SELECT` on `batches`. The shopping list calling the plan's
   coverage is fine.
3. **`rules.rs` files don't use axum, Askama, rusqlite or the `Store`.** They
   take plain data and return plain data, so they can be tested without a
   server or a database. The loading sits next to the queries: for example
   `Store::snapshot` reads the stock once for a page, and `Store::plan` loads
   what `plan::rules::coverage` needs and calls it.
4. **The schema stays in one place.** Queries are split by feature, the
   `SCHEMA` list and migrations are not, so the version chain stays readable
   and rule 1 of `AGENTS.md` is easy to check.
5. **The JSON API is its own slice** that calls the features' queries and
   rules. It only turns results into JSON.
6. **No new file kinds.** If a feature needs something the usual files don't
   cover, it goes in the nearest of them until there is a clear reason.
7. **A new feature is a new folder** like the others, with its `routes()`
   merged in `web/mod.rs` and its templates in `templates/<feature>/`.

## What didn't change

- The binary, its size and memory use: modules and folders are free at run
  time.
- The database file and schema v4.
- URLs, page markup, the JSON API, translations.
- Dependencies: nothing was added.

## Steps

1. **Done: the move.** `db.rs`, `web.rs`, `rules.rs` and the jobs were split
   into the folders above in one pull request that only moved code.
2. **Done: rules out of the database.** The `rules.rs` files take plain data.
   `cargo test --test layout` checks rules 1 and 3 and that every feature's
   routes are merged.

## When to revisit

Split into a workspace with several crates when one of these happens: a second
binary (a command-line tool or a mobile-app sync server), rebuilds during
development get slow enough to hurt, or a part such as the rules is wanted on
its own by another program.
