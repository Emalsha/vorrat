# Vorrat design system

This folder holds the Vorrat design system as exported from
[Open Design](https://github.com/nexu-io/open-design). It describes the brand:
colours, type, voice, imagery and layout. `docs/ui-guideline.md` says how
the interface applies it, and `static/app.css` is the only place the values
live in the app.

## What is here

| Path | What it is |
|---|---|
| `DESIGN.md` | The design system itself and the source of truth: palette, type, voice, imagery, layout and posture rules. |
| `brand.json` | The Open Design project file `DESIGN.md` is generated from. Edit the brand in Open Design, not here. |
| `system/tokens.*.json`, `system/variables*.css`, `system/theme.json`, `system/seed.json` | Tokens the Open Design engine derives from the seed colours (`--brand-*`). Reference only, see below. |
| `system/BRAND-SYSTEM.md` | Open Design's notes on those derived files. |

Left out of the export on purpose: the component showcase
(`system/kit.html`, `system/kit.dark.html`), because it is drawn with the
derived tokens below and shows shadows, dashed buttons and colours Vorrat
doesn't use; the generic marketing pieces
(`system/artifacts/`: landing page with pricing, deck, poster, email,
newsletter, form) and the gallery `system/index.html`, because Vorrat has no
marketing site; `SKILLS.md`, a generic how-to whose palette table disagrees
with `DESIGN.md`; `logos/`, copies of `static/icon-*.png`; and
`system/scripts/apply-design-tokens.mjs`, which would overwrite a stylesheet
with the derived tokens.

## Don't use the derived `--brand-*` tokens

The derived tokens come from a generic algorithm and don't match `DESIGN.md`
or the app. Several fail WCAG 2.2 AA: the dark primary `#406d55` is 3.1:1 on
the dark page, black text on Forest is 3.5:1, and Forest on the derived
`primary-bg` `#a5b0a9` is 2.7:1. They also still describe the earlier green
design: `brand.json` and `system/` predate the Bauhaus palette and need a new
export from Open Design. `DESIGN.md` is what the app follows. Never copy `variables.css` into
`static/`.

## How the design maps onto the app

| `DESIGN.md` | `static/app.css` | Light | Dark |
|---|---|---|---|
| Paper (background) | `--bg` | `#eeede9` | `#121212` |
| Card (surface) | `--surface` | `#ffffff` | `#1d1d1c` |
| Ink (foreground, rules, primary button) | `--ink`, `--accent` | `#141414` | `#f0efea` |
| Text on Ink | `--accent-ink` | `#ffffff` | `#121212` |
| Ink 2 (muted) | `--ink-2` | `#4d4c48` | `#b3b1ab` |
| Line (hairlines) | `--line` | `#d9d8d3` | `#343432` |
| Field (control edges) | `--field` | `#6b6a66` | `#8a8984` |
| Track (empty part of a bar) | `--track` | `#d9d8d3` | `#3a3a38` |
| Hover fill of secondary buttons | `--accent-soft` | `#e2e1dc` | `#2c2c2a` |
| Signal Blue (covered, focus ring) | `--ok`, `--focus` | `#1f4fa0` | `#86a8ee` |
| Signal Yellow (soon, low; triangle fill only) | `--yellow` | `#f2b705` | `#f2b705` |
| Signal Red (expired, missing, destructive) | `--crit` | `#c0261d` | `#ff7b6e` |
| Demo and offline band | `--banner` / `--banner-ink` | `#141414` / `#ffffff` | `#f0efea` / `#121212` |
| Spacing 4, 8, 12, 16, 24, 32 px | `--s1` … `--s6` | | |
| Rules and control edges 2 px | `--rule` | | |
| Tap targets 48 / 40 px, rows 56 px | `--target` / `--target-sm` / `--row` | | |
| Type 0.75 / 0.875 / 1.375 / 1.75 / 4 rem | `--t-xs` / `--t-sm` / `--t-lg` / `--t-title` / `--t-xl` | | |
| Display face (Jost, bundled) | `--display` | | |
| Body face (system UI) | `--font` | | |
| Mono (barcodes, codes) | `--mono` | | |

Jost is bundled as `static/fonts/jost.woff2` (Latin and Latin Extended,
weights 400 to 700, about 23 KB) under the SIL Open Font License, which is
next to it as `static/fonts/Jost-OFL.txt`. It is served from
`/static/jost.woff2` and never loaded from the internet.

`cargo test --test ui` checks that the colours in the front matter of
`DESIGN.md` equal the light tokens in `static/app.css`, and that every token
pair meets WCAG 2.2 AA in both themes.

## Updating the design

1. Change the brand in Open Design and export the design system as a zip.
2. Unzip it into a new, empty folder outside the repository.
3. Compare it with this folder, for example
   `diff -ru docs/design <export>`, and read what changed in `DESIGN.md`.
4. Copy the files listed in "What is here" over the ones in this folder.
   Leave out the same files as before.
5. Change the matching tokens in `static/app.css`, light and dark, and this
   mapping table. If `DESIGN.md` names a new value, add a token for it
   instead of writing the value in a rule.
6. Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
   `cargo test`. The `ui` tests fail if `DESIGN.md` and the tokens disagree or
   a colour pair drops below AA. Fix the colour in Open Design rather than
   lowering a contrast minimum.
7. Update `docs/ui-guideline.md` if a rule changed, look at the pages in
   English and German, light and dark, at phone and desktop width, and open
   a pull request with screenshots.
