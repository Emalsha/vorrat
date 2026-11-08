# Database layer: why plain SQL on rusqlite

*Decided November 2026. Read this before suggesting an ORM.*

## What Vorrat uses

The store is in `src/db/` and each feature's queries are in its own
`queries.rs` (see `docs/ARCHITECTURE.md`). It uses
[rusqlite](https://crates.io/crates/rusqlite) with SQLite compiled into the
binary (the `bundled` feature) and hand-written SQL:

- The schema is the `SCHEMA` list. Each entry is one version, and
  `PRAGMA user_version` records which ones have run. The migration loop also
  seeds a new database and refuses a database made by a newer Vorrat.
- One connection writes; a few read-only connections serve page loads in
  parallel (WAL mode).
- The nightly backup uses SQLite's online backup API, and the JSON export
  reads `SELECT *` from each table without knowing its columns in advance.

## Options we compared

We built the same small program (open a database, create a table, insert,
select) with each option, using Vorrat's release profile. Size is what the
library adds on top of a program that only uses tokio; build time is a clean
release build on 4 cores.

| Option | Kind | Size added | Crates | Clean build |
|---|---|---|---|---|
| rusqlite (current) | Plain SQL | +1.1 MB | 22 | 36 s |
| Diesel 2.3 | ORM, query builder | +1.1 MB | 42 | 48 s |
| SQLx 0.8 | Plain SQL, checked at compile time | +1.4 MB | 150 | 65 s |
| SeaORM 1.1 | ORM on top of SQLx | +1.9 MB | 191 | 66 s |

These are numbers for a toy program. With Vorrat's hundred or so queries,
Diesel's generated code would add more. All four compile SQLite from C, so
none makes the 64-bit ARM build harder or easier.

## Why we stay with rusqlite

- **No size or speed gain.** Every alternative is the same size or larger
  and pulls in more crates, against the Raspberry Pi rule in `AGENTS.md`.
- **SQLx and SeaORM can't move in step by step.** SQLx 0.8 pins an older
  `libsqlite3-sys` than rusqlite 0.37, and only one copy of SQLite can be
  linked, so switching would be one large rewrite. SQLx's compile-time
  checks also need a database or a cached query folder at build time, which
  is the kind of build step the project avoids.
- **Diesel doesn't fit the existing databases.** Its migrations track versions
  in their own table, not in `user_version`, so households' v1 to v4
  databases (and files from the Python prototype) would need bridging code.
  It has no online backup API and no easy way to read rows whose columns
  aren't known in advance, so rusqlite would have to stay for those anyway.
- **The migration loop is 30 lines** and does exactly what we need.
  A migration crate such as `rusqlite_migration` would replace it with a
  dependency that does the same thing.

## How we keep it safe without an ORM

What an ORM really adds is query checking at compile time. Instead:

- Row readers read columns **by name** (`r.get("expires_on")`), not by
  position, so reordering a `SELECT` or adding a column can't shift values
  into the wrong field.
- `tests/rows.rs` runs every read query against a household where each
  optional column is filled in somewhere and empty somewhere else. A query
  that no longer matches the schema, or a column read into the wrong Rust
  type, fails there. When you add a reader to `Store`, add it to that test.

## When to look again

If Vorrat ever needs a second database engine, or the queries grow to the point
where hand-written queries are a real source of bugs, compare again. Diesel
would be the first candidate, because it can live next to rusqlite while
queries move over one at a time.
