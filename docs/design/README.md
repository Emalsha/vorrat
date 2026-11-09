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
`primary-bg` `#a5b0a9` is 2.7:1. `DESIGN.md` was measured from
`static/app.css` and is what the app follows. Never copy `variables.css` into
`static/`.

## How the design maps onto the app

| `DESIGN.md` | `static/app.css` | Light | Dark |
|---|---|---|---|
| Paper (background) | `--bg` | `#f5f4f0` | `#121311` |
| Card (surface) | `--surface` | `#ffffff` | `#1c1d1b` |
| Ink (foreground) | `--ink` | `#1d1c1a` | `#ecebe7` |
| Ink 2 (muted) | `--ink-2` | `#5c5851` | `#a8a49c` |
| Line (border) | `--line` | `#e4e1da` | `#2c2d2a` |
| Field (control edges) | `--field` | `#8c877e` | `#77736b` |
| Track (progress) | `--track` | `#d6d2ca` | `#3a3b38` |
| Forest (accent) | `--accent` | `#2f6f4f` | `#6cc095` |
| Text on Forest | `--accent-ink` | `#ffffff` | `#0d1a13` |
| Forest Soft | `--accent-soft` | `#e8f1ec` | `#1d2b23` |
| Amber / Amber Soft | `--warn` / `--warn-soft` | `#8a5a00` / `#fff3d6` | `#ffcc66` / `#352a14` |
| Brick / Brick Soft | `--crit` / `--crit-soft` | `#b3261e` / `#fbe9e7` | `#ff8a80` / `#3a1d1b` |
| Slate (demo, offline notes) | `--banner` / `--banner-ink` | `#2f4f6f` / `#ffffff` | same |
| Focus Blue | `--focus` | `#1f5fa8` | `#8ab4ff` |
| Spacing 4, 8, 12, 16, 24, 32 px | `--s1` … `--s6` | | |
| Radius 14 / 10 / 6 / 999 px | `--radius` / `--radius-sm` / `--radius-xs` / `--radius-pill` | | |
| Tap targets 44 / 40 px, rows 56 px | `--target` / `--target-sm` / `--row` | | |
| Type 0.75 / 0.875 / 1.25 / 1.5 / 2.25 rem | `--t-xs` / `--t-sm` / `--t-lg` / `--t-title` / `--t-xl` | | |
| Body and display face (system UI) | `--font` | | |
| Mono (barcodes, codes) | `--mono` | | |

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
