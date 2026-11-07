# Vorrat: plan

*A living document: change it as we learn.*

## 1. The goal in one sentence

An open-source, privacy-first app that tells a European household **how ready it
is for a crisis** (power cut, flood, supply disruption) and **keeps it ready**,
fast enough to run on a Raspberry Pi and simple enough for anyone with a browser.

**Runs on any small local machine, offline by default.** The target is the
cheapest box a household already has or can buy: a Raspberry Pi, an old laptop,
a NAS. That is why it is written in Rust. The service must stay as small as
possible (one binary, one SQLite file, a few MB of memory) and must work with no
internet at all: nothing it needs at start-up or for daily use leaves the
machine. Online extras (barcode lookup, push) are optional and fail quietly.

## 2. Why now

- Governments are asking people to prepare. The EU's Preparedness Union Strategy
  (March 2025) asks citizens to be able to manage on their own for at least
  72 hours. National advice goes further: Germany's BBK suggests 10 days,
  Sweden mailed "If crisis or war comes" to every household, and Switzerland,
  Austria, Finland and the Netherlands publish their own checklists.
- The advice is a **static checklist**. Nothing turns it into "what do *I* have,
  for *my* household, and what expires next month". People buy a box of
  supplies once, then it quietly expires in the cellar.
- People who care about preparedness tend to also care about privacy and not
  depending on a cloud that may be down in exactly the crisis they prepare for.

## 3. What already exists

| Product | What it is | Gap for our use |
|---|---|---|
| **Grocy** (open source, PHP) | Full "ERP for the home": stock, recipes, chores | Powerful but heavy and general; emergency planning is not a concept; PHP stack |
| **Homebox** (open source, Go) | Home inventory of *things* (warranties, assets) | No consumables, expiry or emergency targets |
| **KitchenOwl, Mealie, Tandoor** (open source) | Shopping lists, recipes, meal plans | Food-focused, no emergency plan or stock coverage |
| **PrepShield, Provision Planner, Storm Ready, Blackout Prepper** (iOS, closed) | Prepper / emergency-kit apps | Phone-only, closed, mostly US-centred; data on the vendor's servers or locked to one phone |
| **Pantry Check, NoWaste, Out of Milk** (closed) | Pantry and expiry trackers | Food waste angle, no emergency targets, cloud accounts |
| **BBK "Vorsorgen für Krisen" guide, Notvorrat calculators** | Official advice, one-off calculators | No tracking, no reminders |

**Where Vorrat is different:** it is the only one that combines (1) emergency
targets from official national advice, scaled to the household, (2) real
batch-level stock with expiry and first-expiry-first use, and (3) self-hosting
with no account and no telemetry, open source. It is deliberately *narrower*
than Grocy: one job, done very well.

## 4. Product principles

1. **Readiness is the headline.** The first screen answers "how many days could
   we manage?" and "what's missing?". Pantry tracking serves that.
2. **Data entry must be nearly free.** Scan, date, done. Suggestions only when
   they are confident (lesson from Vorrat v0.2: a wrong suggestion hurts more
   than none).
3. **Works in a crisis, and offline by default.** No internet needed to start,
   read or change your data; online extras (barcode lookup, push) are optional
   and give up quietly. Printable checklist. The browser keeps working offline
   through the service worker.
4. **Small enough for a Raspberry Pi.** One binary, one SQLite file, a few MB of
   memory, builds for 64-bit ARM. Every new dependency has to earn its size.
5. **Private by default.** Your stock list says when you're away, what medicine
   you take, what documents you own. It stays on your device or your server.
6. **Local by design.** Country presets (targets, units, language, date and
   decimal format), not a US app translated.

## 5. Stack and architecture

| Layer | Choice | Why |
|---|---|---|
| Language | Rust (2021 edition, MSRV 1.88) | Speed, memory safety, one static binary |
| Web | axum + tokio | Mature, small, async |
| Storage | SQLite via rusqlite (bundled) | One file, zero ops, easy backup; the schema (v2) is stable |
| Pages | Askama templates, compiled into the binary | Type-checked, auto-escaped HTML; works without JavaScript |
| JS | Progressive enhancement only (barcode scanner, quick date entry) | Core works with JS off; strict CSP |
| API | JSON under `/api/v1` | For a future mobile app and integrations (Home Assistant) |
| Packaging | Single binary, Docker image (distroless, non-root), later .deb / Pi image | Easy self-hosting |

**Security baseline (in v0.1 already):** strict Content-Security-Policy, no
framing, `nosniff`, no referrer; same-origin check on every write (CSRF);
redirects only to own pages; all SQL parameterised; input validation for
quantities, units, dates; loopback-only by default. **Next:** accounts with
Argon2id passwords and passkeys, rate limiting, signed release builds, `cargo
audit` and `cargo deny` in CI, a `SECURITY.md` with a disclosure address.

## 6. Licence

**AGPL-3.0-or-later.** Anyone can use and self-host it free; a company that runs
a modified version as a service has to share its changes, so nobody can take the
code closed and compete with the hosted offer. It still allows a paid hosted
version run by us. If we ever want dual licensing (selling a non-AGPL licence to
a business), contributors sign a simple CLA or DCO from the start. Recommendation:
**DCO** (sign-off on commits) now; decide on dual licensing later.

## 7. Roadmap

**Done (v0.1 to v0.4, November 2026).** Stock and batches with first-expiry-first
use, low stock, emergency plan with coverage and "weakest essential lasts N
days", plan presets (BBK 10 days, EU 72 hours, custom), printable checklist,
power-cut view, shopping list from gaps, barcode scanning (bundled ZXing) with
Open Food Facts lookup, locations, documents and attachments, undo, price
history and monthly spend, conservative learned hints, daily ntfy digest,
nightly backup, iCal feed, JSON API with optional token, CSV/JSON/SQLite export
and CSV import, offline service worker, optional household password, demo mode,
English, German, French and Dutch, CI with a 64-bit ARM build.

**Next.**
1. **Household profile: people and pets** (see below).
2. **Suggestions and quick-add from the profile** (see below).
3. **Medicine and other essentials** (see below).
4. **Official guidance in the project:** EU and national emergency-preparedness
   advice, with sources, in `docs/` and as country presets.
5. More languages (Polish, Swedish, Italian, Spanish) and more country presets.
6. Several households on one server: see [HOUSEHOLDS.md](HOUSEHOLDS.md).
7. Accounts with Argon2id passwords and passkeys.
8. Signed release builds and a one-command install, including a Raspberry Pi image.
9. `cargo audit` and `cargo deny` in CI.

**Later.** Native app (as an API client), Home Assistant integration, shared
neighbourhood or building stock, water rotation reminders.

### Household profile, suggestions, medicine and essentials (planned)

Today the plan knows one number: how many people. Real households differ: a
baby needs nappies and formula, a dog needs food and water too, a grandparent
needs their tablets. These three features make the plan fit the household.

**1. Household profile: people and pets.**
- *People:* a first name or nickname, an age group (baby under 2, child 2 to
  12, teenager, adult, older adult), and optional needs: pregnant or
  breastfeeding, diet (vegetarian, vegan, halal, kosher, gluten-free),
  allergies, mobility or care needs, glasses or hearing aids.
- *Pets:* kind (dog, cat, small animal, bird, other), a name, and size or
  weight for dogs.
- The household size used by the plan comes from the profile instead of a
  single "people" number. Each target says how it scales: per adult, per
  person of any age, per baby, per pet, or once per household.
- Quantities scale only where a public source gives a number (for example the
  BBK's per-person amounts). Where no source does (pet food, children's
  portions), Vorrat asks the household to set the amount once and remembers it.
  No invented numbers (AGENTS.md rule 7).

**2. Suggestions and quick-add from the profile.**
- Each profile entry switches on a short checklist of items that official
  guides name for it. Examples: baby: nappies, formula or baby food, wipes,
  a thermometer. Dog or cat: food, water, a lead or carrier, litter,
  vaccination papers, a recent photo. Older adult: spare glasses, hearing-aid
  batteries, a list of medicines. Diet: foods that fit it.
- Suggestions appear on the plan page as "Suggested for your household". One
  tap adds an item to the plan (with a target) or to the shopping list.
  *Not needed* hides it for good. Every suggestion names its source.
- The lists come from the guidance gathered in item 4. They live in plain text
  files in the repository, like the translations, so people can review and
  extend them.

**3. Medicine and other essentials.**
- *Medicine:* a new item kind with its strength and form (tablet, liquid,
  inhaler), who it is for (a person or pet from the profile), the daily dose,
  and an optional prescription renewal date. Vorrat shows how many days each
  person's medicine lasts, counts it in "weakest essential lasts N days", and
  reminds before it runs out or expires. Vorrat never suggests a medicine or a
  dose: that comes from the household and their doctor. The page says so.
- *First-aid kit:* a checklist from official guidance (plasters, bandages,
  disinfectant, painkillers, thermometer, tweezers), each with expiry.
- *Other essentials:* hygiene (soap, toilet paper, menstrual products, rubbish
  bags), light and power (torch, batteries, power bank, battery or wind-up
  radio), cooking without power (camping stove, fuel, matches), cash in small
  notes, copies of documents, and warm clothes and blankets. These become plan
  targets counted "once per household" or "per person", not per day.
- Medicine and health needs are the most private data in Vorrat. They stay in
  the household's own file, are never sent anywhere, and the export marks them
  clearly.

**What this needs.** New tables for people, pets and per-person medicine. That
means schema v3 and a migration (AGENTS.md rule 1), which is a design decision
for the maintainer before anyone builds it. Existing v2 databases upgrade
automatically on first start; the household count becomes adults in the new
profile. Build order: profile first, then medicine and essentials, then
suggestions once the guidance lists exist.

## 8. Risks

- Small, seasonal interest that spikes with news. Mitigation: daily value (food
  waste, expiry) keeps people using it between crises.
- Data entry fatigue. Mitigation: scanning, presets, confident suggestions only.
- Liability: we give planning help, not official advice. Cite sources, link to
  the national authority, add a disclaimer.
- One maintainer. Mitigation: clean code, tests, docs, welcoming contributors.
