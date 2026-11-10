# Vorrat UI guideline

How Vorrat looks and behaves, and why. Read it before changing a template or
`static/app.css`. `AGENTS.md` still wins where the two differ; this file adds
the interface rules on top of it.

The brand itself (palette, type, voice, imagery, layout) is the design system
in `docs/design/`, exported from Open Design. `docs/design/DESIGN.md` is its
source of truth, and `docs/design/README.md` maps it onto the tokens in
`static/app.css` and explains how to bring in a new export. This guideline
applies that design to Vorrat's screens. When you build or change a page,
read `DESIGN.md` for how it should look and sound, and this file for how it
must behave. Where they differ, this file wins, and the difference should be
fixed in Open Design so the next export agrees.

The guideline started from a first draft researched with another assistant
(Grok). Every claim in that draft was checked against the sources listed at the
end. Where this version keeps, corrects or drops something from the draft, the
section "Differences from the first draft" says so.

## 1. What the interface is for

Vorrat is an instrument panel for a household's readiness, not a consumer app
that wants attention. People open it for a few seconds to see whether
something needs doing, and now and then for longer to add stock or adjust the
plan. On a bad day (a power cut, a storm warning, a sick child) they open it
tired, stressed, in poor light, with one hand, maybe on an old phone with a
cracked screen and a weak battery.

So every screen answers three questions in this order:

1. Are we all right? (days covered, overall)
2. What needs doing? (expired, expiring, running low, missing)
3. How do I do it? (one obvious action per row)

## 2. Principles

These follow the seven interaction principles of ISO 9241-110:2020 and the
first-responder interface guidance in ITU-T F.760.2 (2024), adapted for a
household rather than a control room.

| Principle (ISO 9241-110) | What it means in Vorrat |
|---|---|
| Suitability for the user's tasks | The most important number is first and biggest. Nothing on a screen is there for decoration. |
| Self-descriptiveness | Every status says what it is in words ("Expired 3 days ago", "Low"), not only a colour or a number. |
| Conformity with user expectations | Same navigation, same places, same words on every page. A button looks like a button; a link goes somewhere. |
| Learnability | Plain labels, no jargon, no gestures to discover. Empty states say what to do next. |
| Controllability | Every change can be undone or edited. Nothing happens without a tap. Nothing moves or disappears on its own. |
| Use error robustness | Forms keep what was typed when they fail, accept "1,5" and "1.5", and say in one sentence what to fix. |
| User engagement | Earned by being useful daily (expiry, shopping), never by streaks, badges or nagging. |

From stress and disaster-interface research:

- **Stress narrows attention.** Under stress people read less, scan for the
  first thing that looks relevant and miss what is off to the side. Put the
  critical information at the top, in one column, in short sentences.
- **Colour means "look here".** The calm state is quiet. Only things that
  need action get colour. If everything is coloured, nothing stands out.
- **Fewer, larger choices.** One primary action per screen or row. Secondary
  actions are visibly secondary.
- **Nothing alarmist.** No red full-screen banners, sirens, flashing or
  countdowns. Vorrat informs; it doesn't frighten.
- **Plain language.** Short words, active voice, numbers with units ("9 of
  56 L"), dates people read ("3 days ago", "12 Mar 2027").

## 3. Accessibility target

**WCAG 2.2 Level AA**, which is also what the European standard
EN 301 549 V4.1.1 (published 2 September 2026) requires for web content. That
version is expected to become the harmonised standard under the European
Accessibility Act once it is cited in the Official Journal. Vorrat is not
legally bound by the Act (it is not a commercial service), but households in
the EU should get at least what the law asks of shops and banks.

The criteria that matter most for Vorrat, including the ones WCAG 2.2 added:

| Criterion | Level | Rule here |
|---|---|---|
| 1.4.3 Contrast (minimum) | AA | Text 4.5:1, large text (24 px, or 18.66 px bold) 3:1. Checked by `tests/ui.rs`. |
| 1.4.11 Non-text contrast | AA | Field borders, focus rings, progress fills, checkboxes: 3:1 against what is next to them. Decorative dividers are exempt. |
| 1.4.1 Use of colour | A | Status always has an icon and a word as well as a colour. |
| 1.4.4 Resize text, 1.4.10 Reflow | AA | Works at 200 % text size and at 320 px width without sideways scrolling. Use `rem`, wrap long names. |
| 1.4.12 Text spacing | AA | No fixed heights on text containers. |
| 2.1.1 Keyboard | A | Everything works with a keyboard. Use real `<a>`, `<button>`, `<details>`. |
| 2.4.7 Focus visible | AA | A 3 px outline in `--focus` on `:focus-visible`, never removed. |
| 2.4.11 Focus not obscured (new in 2.2) | AA | The sticky header and bottom navigation must not cover the focused element: `scroll-padding` on `html` keeps it clear. |
| 2.5.8 Target size (new in 2.2) | AA | Hard floor 24 × 24 px. Vorrat's own rule is stricter: 48 px for anything used often, 40 px for small buttons. |
| 2.5.7 Dragging movements (new in 2.2) | AA | No drag-only interactions. |
| 3.2.3 Consistent navigation, 3.2.6 Consistent help (new) | AA / A | Bottom navigation (sidebar on a desktop) and settings link in the same place on every page. |
| 3.3.1 Error identification, 3.3.2 Labels | A | Visible labels (not only placeholders). Errors in words, near the field. |
| 3.3.7 Redundant entry (new in 2.2) | A | Don't ask twice for what Vorrat knows: prefill from barcode, last purchase, the household profile. |
| 3.3.8 Accessible authentication (new in 2.2) | AA | The household password field allows paste and password managers (`autocomplete="current-password"`). No puzzles. |
| 4.1.2 Name, role, value | A | Icon-only links have `aria-label`. The current tab has `aria-current="page"`. |
| 4.1.3 Status messages | AA | Confirmation messages use `role="status"`. |

Beyond WCAG, Vorrat also honours the user's system settings:
`prefers-color-scheme` (dark mode), `prefers-reduced-motion` (no animation)
and `forced-colors` (Windows high contrast: bars and status keep visible
borders).

## 4. Running on small machines and offline

Vorrat must be fast on a Raspberry Pi serving an old phone over Wi-Fi, with
no internet at all.

- **Server-rendered HTML first.** Every page is complete without JavaScript.
  JavaScript only adds what a form can't do (camera scanning, offline notice).
- **No fonts from the internet, no icon fonts, no CDNs.** Body text uses the
  system font stack, which is legible and costs nothing. Headings use one
  bundled file, Jost (`static/fonts/jost.woff2`, about 23 KB, SIL Open Font
  License), compiled into the binary and cached by the service worker; if it
  fails, Futura or the system face takes over. Icons are small inline SVG in
  the templates.
- **One stylesheet**, `static/app.css`, compiled into the binary and cached by
  the service worker. Keep it small: today about 20 KB uncompressed. No CSS framework, no
  build step, no preprocessor.
- **No inline styles.** The content security policy is `style-src 'self'`, so
  a `style="…"` attribute does nothing. Use a class. `tests/ui.rs` fails on
  inline styles.
- **Comments only at the top** of a file, function or struct. None inside
  CSS rules, templates or attributes, and no pull request or issue numbers.
- **Native elements over custom widgets:** `<progress>` for bars, `<details>`
  for things to open, `<select>` and `<input type="date">` for choices.
- **Print** works for the plan and checklist, so the paper copy exists before
  the power goes.

## 5. Design tokens

All colours, sizes and spacing come from custom properties at the top of
`static/app.css`. Don't write a colour, radius, size or font anywhere else.
Light values are in `:root`; dark values in the `prefers-color-scheme: dark`
block below it. The values come from `docs/design/DESIGN.md`; the names used
there (Paper, Card, Ink, the signal colours) are mapped in `docs/design/README.md`.
Use the tokens, never the derived `--brand-*` variables in
`docs/design/system/`, which fail contrast.

### Colour

| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` | `#eeede9` | `#121212` | Page background |
| `--surface` | `#ffffff` | `#1d1d1c` | Header, navigation, inputs, hovered rows |
| `--ink` | `#141414` | `#f0efea` | Text, headings, rules, control edges |
| `--ink-2` | `#4d4c48` | `#b3b1ab` | Secondary text (7.3:1 or better); hover of the solid button |
| `--line` | `#d9d8d3` | `#343432` | Hairlines between rows only, never a control's edge |
| `--field` | `#6b6a66` | `#8a8984` | Borders of inputs and tick boxes (4.8:1 or better) |
| `--track` | `#d9d8d3` | `#3a3a38` | Empty part of a bar |
| `--accent` | `#141414` | `#f0efea` | The solid button, the Add tab, filled day squares, bar fill |
| `--accent-ink` | `#ffffff` | `#121212` | Text on `--accent` |
| `--accent-soft` | `#e2e1dc` | `#2c2c2a` | Hover fill of secondary buttons and chips |
| `--ok` | `#1f4fa0` | `#86a8ee` | Covered: the circle and its word |
| `--yellow` | `#f2b705` | `#f2b705` | Soon and low: only the fill of the triangle, never text |
| `--crit` | `#c0261d` | `#ff7b6e` | Expired, missing, destructive: the square, its word, danger buttons |
| `--focus` | `#1f4fa0` | `#86a8ee` | The focus ring |
| `--banner` / `--banner-ink` | `#141414` / `#ffffff` | `#f0efea` / `#121212` | Demo and offline notes |

Every text pair is at least 5:1, more than the 4.5:1 AA asks, so a dim screen
or a cheap panel still reads. `tests/ui.rs` checks the pairs in both themes.
Yellow is 1.8:1 on white, so it never carries text and the triangle always
has an ink edge.

Ink is the action and blue is "covered". The earlier green meant both the
primary button and the covered state; now the most common thing to press and
the calmest state no longer share a colour.

### Size and space

| Token | Value | Use |
|---|---|---|
| `--s1` … `--s6` | 4, 8, 12, 16, 24, 32 px | Gaps and padding. Nothing in between. |
| `--rule` | 2 px | Section rules, the header and tab bar edges, buttons, chips, fields. Corners are always square. |
| `--target` | 48 px | Minimum height of buttons and fields |
| `--target-sm` | 40 px | Small buttons and chips |
| `--row` | 56 px | Minimum height of a list row |
| `--measure` | 720 px | Maximum content width on a phone or tablet, and of a lone form |
| `--wide` | 1160 px | Maximum content width on a desktop |
| `--sidebar` | 248 px | Width of the desktop sidebar |
| `--t-xs`, `--t-sm`, `--t-lg`, `--t-title`, `--t-xl` | 0.75, 0.875, 1.375, 1.75, 4 rem | Tab labels and section headings; small text; titles; desktop titles; the big number. Body is 1 rem (16 px), line height 1.45. No text below 0.75 rem. |
| `--display` | Jost, bundled | Page titles, section labels, the big number, counts. One 23 KB file served by Vorrat, never from the internet. |
| `--font`, `--mono` | system UI, system monospace | Body text, labels, buttons; barcodes and codes. |

### Surfaces and states

From `DESIGN.md`:

- Rules, not boxes: a `.card` is a 2 px ink rule above its rows, on `--bg`.
  No shadows, gradients, elevation or rounded corners.
- Links are ink and always underlined; hover thickens the underline. Hover
  fills secondary buttons with `--accent-soft`, rows with `--surface`, and
  turns the solid button `--ink-2`. Never fade text to grey to show a state.
- The current item has `aria-current` and a cue that isn't colour (a 4 px
  bar and bold text).
- Nothing moves on its own: no animation, fading or toasts.

### Icons and pictures

- Icons are inline SVG in the templates: 24 × 24 viewBox, `stroke="currentColor"`,
  stroke width 2, no fills. No emoji, icon fonts or icon CDNs.
- Status is a filled shape from `_icon.html`: a circle in `--ok`, a triangle
  in `--yellow` with an ink edge, a square in `--crit`. Always followed by a word.
- Pictures are the household's own: product photos from a barcode lookup or
  an upload, scanned documents. Thumbnails are 42 px squares (64 to 88 px on
  forms and the item page), square, `object-fit: cover`. Without a photo
  there is no thumbnail.
- No stock photos, illustrations or alarm imagery. The logo is the circle,
  triangle and square on paper above an ink band, in `static/icon-*.png`.

## 6. Layout shell

Every page uses `templates/base.html`. One page serves every screen: the
same HTML, laid out by CSS for a phone or for a desktop. There is no second
template, no user-agent sniffing and no JavaScript involved.

Common to both:

1. A "Skip to content" link, the first thing a keyboard reaches.
2. A sticky header with the page title. Pages below a list (an item, its
   edit form) put a back link before the title in the `back` block.
3. `<main id="main">`, the message from the last action (with Undo when
   there is one) at its top. It stays until the next page; it doesn't fade
   away while someone is reading or reaching for Undo.
4. One `<nav class="tabs">` that holds every place. Links marked
   `.phone-only` show only on a phone, links marked `.wide-only` only on a
   desktop, so the DOM order is the focus order in both layouts.

### Phone and tablet (below 64em, about 1024 px)

- One column, at most `--measure` wide, 16 px gutters.
- The fixed bottom navigation with five places: Home, Stock, Add, Plan,
  Shopping. Add sits in the middle as a solid ink cell in the same grid,
  within reach of a thumb. The current place has `aria-current="page"`, a
  4 px ink bar above it and bold text.
- Settings is the gear in the header.
- Secondary information (files, barcodes, prices, history) sits in
  `<details>` so the first screen shows what is used every day.

### Desktop (64em and wider)

- A fixed sidebar, `--sidebar` wide, on the left: the Vorrat name, Add
  stock as the one primary button, the four places, a divider, then
  Medicine and essentials, Household, Locations, and Settings at the bottom.
  The current place has a paper background, bold text and a 4 px ink bar on
  its left edge, so it never depends on colour alone.
- Content up to `--wide`, 24 px gutters. The header has no gear: Settings
  is in the sidebar.
- From 75em (about 1200 px) pages use two columns with `.cols` (below).
  Under 75em, and at 200 % text zoom, they fall back to one column because
  the breakpoints are in `em`.

### Page columns

`.cols > .col + .col` is one column on a phone and two on a wide screen.
The DOM order is the phone order, so put what matters first in the first
column, and the side information in the second.

| Variant | Columns | Use |
|---|---|---|
| `.cols` | 3 : 2 | Main content and a side column (Home, Plan, Item, Add, Scan, Shopping) |
| `.cols.lead` | narrow first, sticky | Filters next to a long list (Stock) |
| `.cols.even` | 1 : 1 | Groups of equal weight (Settings) |

`.narrow` keeps a lone form at `--measure` on a wide screen (edit item).

## 7. Components

Use these classes; add a new one only when none fits, and add it here.

| Component | Markup | Notes |
|---|---|---|
| Card | `.card` (+ `.pad`) | A section: a 2 px ink rule above its content. No box, no shadow. |
| Section heading | `<h2>` | Jost, small, uppercase, ink. One per group; may start with a status shape. |
| List | `.card > ul.list > li` | Full-width rows divided by `--line`. |
| Row | `a.row` or `div.row` with `.grow > .title + .sub` and an end element | At least `--target` high. Whole row is the link. Long names wrap on the item page and truncate in lists. |
| Status | `.status.ok`, `.status.warn`, `.status.crit` | Icon + words. Include `_icon.html` for the icon. Used inline in text. |
| Pill | `.pill.expired`, `.pill.urgent`, `.pill.soon`, `.pill.low` | Short status: a shape and a word, no fill. |
| Count | `.count` with a `<small>` unit | The quantity at the end of a stock row, in Jost with tabular figures. |
| Day squares | `.days.n14 > i.f` | One square per planned day (up to 31), filled for each whole day covered. `day_cells` in `web/view.rs` builds them. |
| Progress | `<progress class="bar" max="100" value="…">` | Ink on track, square ends. For targets not counted in days, or plans over 31 days. The numbers are always written next to it. |
| Readiness | `.ready` | The big "N of D days" with its day squares on Home and Plan. |
| Button | `.btn`, `.btn.primary`, `.btn.danger`, `.btn.small`, `.btn.block`, `.btn.big` | One `.primary` per group. Labels are verbs ("Add stock", "Used up"). |
| Chips | `nav.chips > a.chip` (`aria-current` on the chosen one) | Filters as links, so they work without JavaScript. |
| Field | `<label>` wrapping text + `<input>` | Visible label. Placeholder only as an example ("e.g. 1.5"). |
| Empty state | `li.empty` | Says what's true and what to do next. |
| Notice | `.notice` (`role="status"`) | The message after an action, with an optional Undo. |
| Disclosure | `<details>` with a `<summary>` | For editing and for secondary information. |

Helpers: `.phone-only`, `.wide-only` (shown only below or above 64em),
`.cols`, `.narrow`, `.muted`, `.small`, `.right`, `.mt`, `.mt-xs`, `.m0`, `.mt0`, `.pt0`,
`.flex`, `.row-line` (a row with controls side by side), `.row.col` (a row
that stacks), `.between`, `.end`, `.grow`, `.grid2`, `.grid3`, `.stack`, `.vh` (visually hidden,
still read aloud).

### Status vocabulary

The same four words and icons everywhere:

| State | Icon | Colour | Examples |
|---|---|---|---|
| Covered / fine | circle | `--ok` | "Covered", "Nothing needs attention." |
| Soon | triangle | `--yellow` shape, ink text | "In 5 days", "Low", "6 of 14 days" |
| Expired / missing | square | `--crit` | "Expired 3 days ago", "Missing 12 L" |
| No date | none | `--ink-2` | "no date" |

## 8. Screens

### Home

On a desktop, readiness and the scan buttons are the first column and
"Needs attention" with recent activity the second.

1. **Readiness**: "Ready for N of D days" in the biggest type on the page,
   where N is how long the weakest essential lasts (capped at D), one square
   per planned day, people in the household, and what isn't covered yet. The
   whole card links to Plan. The percentage is on Plan only.
2. **What needs doing**, grouped by what you do: **Expired** (one "Thrown
   away" button per row), **Use first** (expiring within 14 days, one "Used
   up" button per row), **Running low** (with the count). The other choice is
   on the item page. When nothing needs attention, say so with a circle.
3. **Scan to add** and **Scan to use up**, the two everyday actions.
4. **Recent activity**, closed by default.

### Stock

1. Search field, always visible, with a visible button (works without
   JavaScript).
2. Filter chips: All, Food, Supplies, Documents, Things, Running low.
3. Sort as links: by name or by expiry.
4. Rows: name, minimum, category and next expiry, a pill when it is low or
   expiring, and the count with its unit on the right. The number of items shown.
5. Adding is the middle tab, so there is no second floating Add button.
6. On a desktop the search, filters and the Locations link stay in a sticky
   column on the left; the list fills the rest.

### Plan

1. The same readiness card as Home, with the household and the number of days
   to plan for, and the official presets for those days.
2. **Coverage**, least covered first: one row per target with its day
   squares (or a bar), the amounts, and its status in words ("Covered",
   "Missing 12 L"). Tap to see what counts towards
   it and to edit the target.
3. "Put the gaps on the shopping list", the main action, right after.
4. Medicine and essentials, suggestions for the household.
5. Tools: printable checklist, power cut.
6. Add a target, closed by default.

### Item

1. The amount in stock in big type, with kind, category and barcode under
   it, and the picture beside it.
2. Use and Thrown away right there, then Add stock and Edit.
3. Batches, then the Add stock form.
4. Side column: daily dose (medicine), minimum, then files, barcodes,
   prices and history in `<details>` (files open for documents), and
   Delete last.

### Add

The fields people fill every time (name, kind, category, quantity, unit,
expiry, location, minimum) first; brand, pack size, barcode and price in
"More details", open when a barcode lookup filled them. The side column
offers scanning, or the "Same thing" matches after a scan.

### Scan

Mode chips and the camera first; typing the code and adding without a
barcode in the side column.

### Shopping

The list with its add field and the bulk actions first; "You may need
soon" in the side column.

### Settings

Language, reminders, calendar and backup in the first column; your data,
privacy and sign out in the second.

## 9. Writing

As in `AGENTS.md`: plain, short, concrete, full sentences, no exclamation
marks. In addition:

- Write numbers with units and "of": "9 of 56 L", "3 of 14 days".
- Use the words in `DESIGN.md` ("Covered", "Low", "Missing 12 L", "Used up",
  "Thrown away") and avoid what it lists: exclamation marks, "Oops",
  alarmist wording, streaks or badges, jargon, error codes.
- Say what to do, not what went wrong inside: "Enter a number, like 1.5."
- Every visible text goes through `|t` or `tr()` and into every
  `locales/*.tsv`. German is often 30 % longer: leave room, let text wrap.

## 10. Checks

Automatic, in `cargo test` (`tests/ui.rs`):

- Contrast of every token pair in light and dark against WCAG AA.
- The colours in the front matter of `docs/design/DESIGN.md` equal the light
  tokens.
- No `style="` attributes in templates.
- No `|safe` in templates.

By hand, for any change people see (say in the pull request what you did):

- Look at the page at 360 px wide, in English and German, light and dark.
- Tab through it with a keyboard: every control reachable, focus visible and
  not hidden under the header or navigation.
- Zoom the text to 200 %: nothing cut off, no sideways scrolling.
- Turn off JavaScript: the page still reads and its forms still work.
- If you can, try a screen reader (VoiceOver, TalkBack or NVDA) on the page.

## Differences from the first draft

Kept: the instrument-panel idea, the order of goals, "never colour alone",
system fonts, 16 px body text, visible labels, empty states that say what to
do, progressive enhancement, Home and Stock first.

Corrected:

- **ISO 9241-110** has had seven principles since its 2020 revision. The draft
  listed five, mixing the 2006 and 2020 names. The table in section 2 uses
  the current seven.
- **Target size.** The draft called 44 × 44 px a WCAG 2.2 AA requirement. AA
  (2.5.8) asks for 24 × 24 px; 44 px is the AAA level (2.5.5). Vorrat uses
  48 px as its own rule, for gloves, shaking hands and small screens.
- **The draft's warning orange `#ED6C02` fails** text contrast: 3.1:1 on
  white. Vorrat uses `#8a5a00` (5.9:1). The draft's `#1A1A1A` is 17.4:1 on
  white, not 15.8:1.
- **Borders.** The draft's `#E0E0E0` border is fine for dividers but fails
  1.4.11 (1.3:1) as the only edge of an input. Fields get their own `--field`
  token.
- **ITU-T F.760.2 (04/2024)** exists and is about first responders' tools; it
  is used here only for general principles (salience of critical information,
  minimal interaction steps), not as a requirement for a household app.

Added: the WCAG 2.2 criteria the draft didn't mention (focus not obscured,
redundant entry, accessible authentication, consistent help), EN 301 549 and
the European Accessibility Act, `prefers-reduced-motion` and `forced-colors`,
a no-inline-style rule backed by the CSP, automated checks, and the screen
details for Plan.

Changed:

- **Five navigation places, not four with "More".** Shopping is used every
  week and keeps people opening Vorrat between crises; hiding it behind
  "More" would cost more than a fifth tab. Settings stays in the header, as
  today. There is no Documents section yet; documents are items of kind
  "document".
- **Green accent, not blue**, for the reason in section 5.
- **No floating Add button** on Stock: the middle tab already is one.
- **Messages after an action stay** instead of fading, so Undo can't vanish
  while someone reaches for it.

## Sources

- W3C, [Web Content Accessibility Guidelines (WCAG) 2.2](https://www.w3.org/TR/WCAG22/), and [What's new in WCAG 2.2](https://www.w3.org/WAI/standards-guidelines/wcag/new-in-22/)
- W3C, [Understanding 2.5.8 Target Size (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html)
- ETSI/CEN/CENELEC, EN 301 549 V4.1.1, summarised by [Deque](https://www.deque.com/blog/en-301-549-v4-1-1-is-final-what-changed-what-it-means-and-what-you-should-do/) and [Davis Wright Tremaine (Sept. 2026)](https://www.dwt.com/insights/2026/09/european-accessibility-act-ict-standards-update)
- ISO 9241-110:2020, Ergonomics of human-system interaction, Part 110: Interaction principles ([catalogue entry](https://standards.iteh.ai/catalog/standards/cen/7cfeb7e3-9a82-44af-871c-116631d1d051/en-iso-9241-110-2020))
- ITU-T [F.760.2 (04/2024)](https://www.itu.int/rec/T-REC-F.760.2-202404-I), Guidelines for user interface of first responders in emergency response support systems
- US CDC, [Crisis and Emergency Risk Communication (CERC)](https://emergency.cdc.gov/cerc/) on simple, consistent messages under stress
- MDN, [prefers-reduced-motion](https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-reduced-motion) and [forced-colors](https://developer.mozilla.org/en-US/docs/Web/CSS/@media/forced-colors)
