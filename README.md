# Vorrat

**Know if your household is ready for a crisis, and stay ready.**

Vorrat (German for *supplies*) is a private, self-hosted app for emergency
preparedness. Tell it how many people live with you and how many days you want
to cover; it shows what you have, what's missing, and reminds you before
anything expires, so your emergency stock is never quietly out of date.

- **Emergency plan first.** Targets start from official civil-protection advice
  (German BBK checklist today; more countries planned) and adapt to your
  household size and the number of days. "Water: 3.8 of 14 days covered."
- **Tracks real stock**, batch by batch with expiry dates. Use-up always takes the
  batch that expires first. Low-stock minimums. Shopping list from the gaps.
- **Private by design.** One binary, one SQLite file on your own machine. No
  account, no telemetry, no cloud. Works without JavaScript.
- **Scan to add.** The phone camera reads the barcode (bundled ZXing, no CDN);
  unknown products are filled in from Open Food Facts (only the barcode is sent).
- **Reminds you.** One daily push via [ntfy](https://ntfy.sh) when something is
  about to expire or runs low; calendar feed for expiry and renewal dates.
- **Documents too.** Passport, insurance, warranties: renewal dates and scans.
- **Small and offline.** Written in Rust so it fits any small local machine,
  such as a Raspberry Pi: one ~7 MB binary, about 13 MB of memory, starts in
  milliseconds. It needs no internet: barcode lookup and push are optional
  extras that fail quietly. Nightly backup built in.

> Status: **early (v0.4)**, in daily use by its author's household. See
> [docs/PLAN.md](docs/PLAN.md) for where it's going.

## Run it

```bash
cargo run --release            # http://127.0.0.1:8087, data in ./data
```

or with Docker:

```bash
docker compose -f compose.example.yaml up -d
```

| Variable | Default | |
|---|---|---|
| `VORRAT_DATA_DIR` | `./data` | holds `vorrat.db` |
| `VORRAT_LISTEN` | `127.0.0.1:8087` | loopback by default; use a TLS proxy for other devices |
| `VORRAT_CURRENCY` | `EUR` | |
| `VORRAT_NTFY_TOPIC` | (off) | ntfy topic for the daily push; long and random on ntfy.sh |
| `VORRAT_NTFY_URL` | `https://ntfy.sh` | or your own ntfy server |
| `VORRAT_NTFY_TOKEN` | | access token, if your ntfy needs one |
| `VORRAT_PUBLIC_URL` | | tapping the push opens this URL |
| `VORRAT_DIGEST_TIME` | `08:00` | local time of the daily push |
| `VORRAT_BACKUP_DIR` | (off) | nightly DB copy + files archive go here |
| `VORRAT_BACKUP_TIME` | `03:30` | |
| `VORRAT_BACKUP_KEEP_DAYS` | `7` | |
| `VORRAT_LOOKUP` | `on` | `off` = never call Open Food Facts |
| `VORRAT_API_TOKEN` | empty | Bearer token required on `/api/` when set |
| `VORRAT_DEMO` | `off` | `on` = public demo: sample household, reset every night, no uploads, imports or settings changes |
| `VORRAT_PASSWORD` | empty | Household password: when set, every page needs a sign-in (sessions last 30 days, a restart signs everyone out) |

There is no login yet: run it on your home network or behind a VPN such as
Tailscale or WireGuard. Built-in accounts are on the roadmap.

For a public server with HTTPS, backups and a Raspberry Pi, see
[docs/SELF-HOSTING.md](docs/SELF-HOSTING.md).

## Language

English, German, French and Dutch. Vorrat follows the browser's language;
Settings → Language fixes it for everyone, including the daily push.

Translations are plain text files in `locales/` (`English<TAB>translation`, one
line per text; the English text is the key). Templates mark text with
`{{ "Stock"|t }}`. Tests fail when a marked text has no German, or when another
language misses a line or a `{0}` placeholder. To add a language, copy
`locales/de.tsv`, translate the right-hand column and add one line to `LANGS`
in `src/i18n.rs`. The French and Dutch files are first drafts: corrections from
native speakers are very welcome.

## Export and import

Settings → Your data: the stock as CSV (opens in any spreadsheet), everything
as JSON, or the SQLite file itself. Import a CSV (comma or semicolon, dates as
`2027-05-01` or `01.05.2027`) to add stock: items are matched by barcode, then
name, and nothing is deleted. The CSV export imports back unchanged.

## JSON API

For scripts, Home Assistant and the like. JSON in, JSON out; errors come back
as `{"error": "..."}` with status 400, 404 or 409.

| Call | What it does |
|---|---|
| `GET /api/v1/items?q=&kind=&in_stock=true` | All items |
| `GET /api/v1/items/{id}` | One item with its batches |
| `GET /api/v1/barcode/{code}` | The item with that barcode (404 if none) |
| `GET /api/v1/expiring?within=14` | Batches expiring within N days |
| `GET /api/v1/plan` | Emergency plan coverage |
| `GET /api/v1/summary` | Counts for a dashboard tile |
| `GET /api/v1/digest` | Preview of today's reminder |
| `GET /api/v1/shopping`, `GET /api/v1/suggestions` | Shopping list, and what you'll likely need |
| `POST /api/v1/items` `{"name":"Rice","unit":"kg","barcode":"..."}` | New item (201, `{"id":..}`) |
| `POST /api/v1/items/{id}/batches` `{"qty":2,"expires_on":"2027-05-01"}` | Add stock |
| `POST /api/v1/items/{id}/consume` `{"qty":1,"waste":false}` | Use up, oldest first |
| `POST /api/v1/shopping` `{"item_id":3,"qty":2}` or `{"label":"Candles"}` | Add to the list |

Set `VORRAT_API_TOKEN` to require `Authorization: Bearer <token>` on every API
call (the pages don't need it). Browsers on other sites can't write: the same
cross-site check as the forms applies.

## Develop

```bash
cargo test         # unit + integration tests, including upgrading a schema v2 database
cargo clippy --all-targets
```

Code map: one folder per feature under `src/` (`stock/`, `plan/`, `shopping/`,
...), each with its routes, pages, queries and rules, on top of shared modules:
`src/db/` (SQLite store, schema and migrations), `src/web/` (router, security
headers, CSRF check), `src/auth/`, `src/jobs/` (daily push, nightly backup),
`src/lookup.rs` (Open Food Facts) and `src/files.rs` (safe file names).
`templates/` holds the Askama pages (compiled into the binary) and `static/`
the CSS, scanner script and bundled ZXing under its Apache-2.0 licence. See
`docs/ARCHITECTURE.md`.

## Licence

[AGPL-3.0-or-later](LICENSE). You can use, change and self-host it freely; if you
offer a modified version as a service, share your changes.
