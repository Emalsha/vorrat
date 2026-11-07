# Design: several households on one server

Status: proposal, not built. Needed for a hosted "Vorrat Cloud" and for
organisations (housing co-ops, offices). Self-hosters keep the single-household
mode with no change.

## Goal

One Vorrat server serves many households. Each household sees only its own
stock, plan, shopping list and files. People sign in as themselves and can
belong to more than one household (a family and a shared holiday home).

## Recommendation: one SQLite file per household

```
data/
  accounts.db              people, sessions, memberships, invites
  households/
    7f3a…/vorrat.db        today's schema, unchanged
    7f3a…/files/
    b912…/vorrat.db
```

Why not one database with a `household_id` column on every table:

- **Isolation by construction.** A missing `WHERE household_id = ?` can't leak
  another household's data, because that data isn't in the file.
- **No schema change.** Each household file is exactly today's schema, so
  existing databases, the export and the current tests keep working.
- **Export and delete are a file.** "Download my data" sends the file; GDPR
  erasure deletes a folder.
- **Backups and restores per household.**

Cost: one open connection pool per active household. Pools are opened on first
use and closed after 10 idle minutes (an LRU of `Store`s), so memory follows
active households rather than all of them. At ~1–2 MB per open pool this fits
thousands of households on a small server.

## Accounts

- `accounts.db`: `people(id, email, password_hash, lang, created)`,
  `households(id, name, created)`, `members(person, household, role)` with roles
  `owner` and `member`, `sessions(token_hash, person, household, expires)`,
  `invites(token_hash, household, expires)`.
- Passwords hashed with Argon2id (new dependency: `argon2`). Sessions stored
  hashed, so a leaked accounts file can't be used to sign in.
- Invites are single-use links that expire after 7 days. No email server
  needed at first: the owner copies the link.
- Sign-up can be turned off (`VORRAT_SIGNUP=off`) for private servers.

## Request flow

1. The session cookie → person and current household.
2. Middleware picks that household's `Store` and puts it in the request
   (replacing today's single `state.store`).
3. Handlers are unchanged apart from taking the store from the request.

The digest and backup jobs loop over households; push settings (ntfy topic)
move into each household's `settings` table.

## Steps

1. Refactor: handlers take a `Store` from a request extension (no behaviour change).
2. `accounts.db`, sign-up, sign-in, sign-out, Argon2id, hashed sessions.
3. Store pool per household, created on sign-up.
4. Invites and a household switcher.
5. Per-household jobs (digest, backup) and settings.
6. Migration: an existing single database becomes the first household of the
   first account.

## Open question

Email sign-in links (needs an email provider) or passwords only at first?
