# Project structure

*Proposal, November 2026. Nothing has moved yet: the code changes start once
Emalsha agrees. Read this before adding a new module or folder.*

## The short answer

Rust has one structure everybody follows, and it is small: the Cargo package
layout (`src/main.rs`, `src/lib.rs`, `tests/`, `benches/`, `examples/`).
Above that there is no house style like Clean Architecture in .NET. Projects
pick from a few common patterns, described below.

For Vorrat we recommend:

- **One crate**, as today. No workspace with several crates yet.
- **Vertical slices**: one folder per feature (stock, plan, shopping...), each
  holding that feature's pages, queries and rules.
- **A thin shared core** for what every feature needs: the database
  connection and schema, the web plumbing (router, security, errors), config
  and translations.
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

## Where Vorrat is today

One crate, layered by technical role:

| File | Lines | Holds |
|---|---|---|
| `src/web.rs` | 3,100 | Router, middleware, every page handler and its template struct, form structs, view helpers, the JSON API |
| `src/db.rs` | 2,100 | Schema and migrations, the `Store`, every query for every table |
| `src/rules.rs` | 650 | Expiry, low stock, plan coverage, digest, power cut |
| `src/essentials.rs` | 510 | Medicine and checklists |
| everything else | under 400 each | Config, auth, jobs, lookup, transfer, demo... |

This was the right shape for a first version. It now has two costs: a change
to the shopping list touches `web.rs`, `db.rs` and a template spread far
apart, and the two big files are where every pull request collides.

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

## Proposed layout

```text
src/
  main.rs              start-up, signals, server
  lib.rs               module list
  config.rs            VORRAT_* settings

  core/                shared by every feature, knows no feature
    db.rs              Store: one writer, pooled readers, settings, log, undo
    schema.rs          SCHEMA list and the migration loop (the only place)
    web.rs             AppState, router assembly, security and language middleware
    error.rs           AppError, flash messages, back/done/safe_back/safe_next
    view.rs            render, Askama filters, date and money helpers
    i18n.rs            tr, trf, Lang
    auth.rs            household password, sessions, API token
    files.rs           safe names, byte sniffing, /files/ and /static/

  stock/               items, batches, barcodes, attachments
    mod.rs             routes() for this feature
    pages.rs           handlers, form structs, template structs
    queries.rs         impl Store { items, batches, consume, ... }
    rules.rs           expiry status, low stock (no HTTP, no SQL)
  locations/
  plan/                coverage, presets, targets, print view, power cut
  household/           people and pets
  essentials/          medicine, doses, checklists
  shopping/
  settings/            language, test push, backup now
  api/                 JSON under /api/v1, calls the same queries and rules

  jobs/                scheduler, notify (ntfy), backup
  smart.rs             learned hints
  lookup.rs            Open Food Facts
  transfer.rs          CSV and JSON export, CSV import
  demo.rs              sample household

templates/             one folder per feature, shared parts at the top
  base.html  _icon.html  ...
  stock/item.html  stock/items.html  stock/add.html ...
  plan/plan.html  plan/print.html ...
```

Each feature folder has the same four files, and a small feature may skip
`rules.rs`. Each feature's `routes()` returns an axum `Router`, and
`core/web.rs` merges them and wraps them all in the same security layers, so
a new page can't miss them.

### Rules for the layout

1. **A feature depends on `core`, never the other way round.** `core` doesn't
   name any feature.
2. **A feature uses another feature only through its `pub(crate)` functions**,
   never its tables. The plan reads stock through `stock::queries`, it doesn't
   write its own `SELECT` on `batches`. The shopping list calling the plan's
   coverage is fine.
3. **`rules.rs` files don't use axum, Askama or rusqlite.** They take plain
   data and return plain data, so tests need no server and no database.
   Today `rules.rs` takes a `&Store` and loads what it needs; when moving, the
   loading goes to the handler or to a `load` function next to the queries.
   A small test in the style of `tests/style.rs` can check this rule.
4. **The schema stays in one place.** Queries are split by feature, the
   `SCHEMA` list and migrations are not, so the version chain stays readable
   and rule 1 of `AGENTS.md` is easy to check.
5. **The JSON API is its own slice** that calls the features' queries and
   rules. It only turns results into JSON.
6. **No new file kinds.** If a feature needs something the four files don't
   cover, it goes in the nearest of them until there is a clear reason.

## What doesn't change

- The binary, its size and memory use: modules and folders are free at run
  time. Each moving step checks the release size stays about 7 MB.
- The database file and schema v4.
- URLs, page markup, the JSON API, translations.
- Dependencies: nothing is added.

## How to get there

Each step is one pull request that only moves code, with no behaviour change,
and passes `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
and `cargo test` on its own.

1. **Core.** Split `db.rs` into `core/db.rs` and `core/schema.rs`, and move
   the shared parts of `web.rs` (state, router, middleware, errors, helpers)
   into `core/`. Feature code stays where it is.
2. **One pilot slice: shopping.** It is small and touches stock and plan, so
   it shows whether the rules above work. We look at it together before going
   on.
3. **The other slices**, one or two per pull request: stock, locations, plan,
   household, essentials, settings, API.
4. **Rules out of the database.** Make the `rules.rs` files take plain data,
   and add the check for rule 3.
5. **Update `AGENTS.md`**: its map, and "a new feature is a new folder".

## When to revisit

Split into a workspace with several crates when one of these happens: a second
binary (a command-line tool or a mobile-app sync server), rebuilds during
development get slow enough to hurt, or a part such as the rules is wanted on
its own by another program.
