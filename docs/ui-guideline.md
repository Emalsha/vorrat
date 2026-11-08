# Vorrat UI guideline

How Vorrat looks and behaves, and why. Read it before changing a template or
`static/app.css`. `AGENTS.md` still wins where the two differ; this file adds
the interface rules on top of it.

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
| 2.5.8 Target size (new in 2.2) | AA | Hard floor 24 × 24 px. Vorrat's own rule is stricter: 44 px for anything used often, 40 px for small buttons. |
| 2.5.7 Dragging movements (new in 2.2) | AA | No drag-only interactions. |
| 3.2.3 Consistent navigation, 3.2.6 Consistent help (new) | AA / A | Bottom navigation and settings link in the same place on every page. |
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
- **No web fonts, no icon fonts, no CDNs.** The system font stack is legible
  and costs nothing. Icons are small inline SVG in the templates.
- **One stylesheet**, `static/app.css`, compiled into the binary and cached by
  the service worker. Keep it small: today about 10 KB. No CSS framework, no
  build step, no preprocessor.
- **No inline styles.** The content security policy is `style-src 'self'`, so
  a `style="…"` attribute does nothing. Use a class. `tests/ui.rs` fails on
  inline styles.
- **Native elements over custom widgets:** `<progress>` for bars, `<details>`
  for things to open, `<select>` and `<input type="date">` for choices.
- **Print** works for the plan and checklist, so the paper copy exists before
  the power goes.

## 5. Design tokens

All colours, sizes and spacing come from custom properties at the top of
`static/app.css`. Don't write a colour anywhere else. Light values are in
`:root`; dark values in the `prefers-color-scheme: dark` block below it.

### Colour

| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` | `#f5f4f0` | `#121311` | Page background |
| `--surface` | `#ffffff` | `#1c1d1b` | Cards, lists, the navigation bar |
| `--ink` | `#1d1c1a` | `#ecebe7` | Body text and headings |
| `--ink-2` | `#5c5851` | `#a8a49c` | Secondary text (still 6.4:1 or better) |
| `--line` | `#e4e1da` | `#2c2d2a` | Decorative dividers only, never a control's edge |
| `--field` | `#8c877e` | `#77736b` | Borders of inputs, checkboxes (3:1 or better) |
| `--track` | `#d6d2ca` | `#3a3b38` | Empty part of a progress bar |
| `--accent` | `#2f6f4f` | `#6cc095` | Primary buttons, links, current tab, "covered" |
| `--accent-ink` | `#ffffff` | `#0d1a13` | Text on `--accent` |
| `--accent-soft` | `#e8f1ec` | `#1d2b23` | Secondary buttons, quiet highlights |
| `--warn` / `--warn-soft` | `#8a5a00` / `#fff3d6` | `#ffcc66` / `#352a14` | Expiring soon, low, partly covered |
| `--crit` / `--crit-soft` | `#b3261e` / `#fbe9e7` | `#ff8a80` / `#3a1d1b` | Expired, missing, destructive actions |
| `--focus` | `#1f5fa8` | `#8ab4ff` | Focus ring, information notes |

Every text pair is at least 5:1, more than the 4.5:1 AA asks, so a dim screen
or a cheap panel still reads. `tests/ui.rs` checks the pairs in both themes.

Vorrat keeps its own green as the accent instead of the draft's blue. Green is
the brand colour of the icon and the existing installs, and "covered" sharing
it with the primary action is harmless because status never relies on colour
alone.

### Size and space

| Token | Value | Use |
|---|---|---|
| `--s1` … `--s6` | 4, 8, 12, 16, 24, 32 px | Gaps and padding. Nothing in between. |
| `--radius`, `--radius-sm` | 14 px, 10 px | Cards; buttons and fields |
| `--target` | 44 px | Minimum height of buttons, rows, fields, tabs |
| `--measure` | 720 px | Maximum content width |
| Type | 16 px body, 0.875 rem small, 1.25 rem titles, 2.25 rem the big number | No text below 0.75 rem, and that only for tab labels |

## 6. Layout shell

Every page uses `templates/base.html`:

1. A "Skip to content" link, the first thing a keyboard reaches.
2. A sticky header: page title on the left, settings on the right.
3. `<main id="main">`: one column, at most `--measure` wide, 16 px gutters.
4. The message from the last action (with Undo when there is one) at the top
   of `main`. It stays until the next page; it doesn't fade away while
   someone is reading or reaching for Undo.
5. The fixed bottom navigation with five places: Home, Stock, Add, Plan,
   Shopping. Add sits in the middle and is the largest. The current place has
   `aria-current="page"` and the accent colour.

## 7. Components

Use these classes; add a new one only when none fits, and add it here.

| Component | Markup | Notes |
|---|---|---|
| Card | `.card` (+ `.pad`) | A white surface with a border. No shadows. |
| Section heading | `<h2>` | Small, uppercase, `--ink-2`. One per group. |
| List | `.card > ul.list > li` | Full-width rows divided by `--line`. |
| Row | `a.row` or `div.row` with `.grow > .title + .sub` and an end element | At least `--target` high. Whole row is the link. Long names wrap on the item page and truncate in lists. |
| Status | `.status.ok`, `.status.warn`, `.status.crit` | Icon + words. Include `_icon.html` for the icon. Used inline in text. |
| Pill | `.pill.expired`, `.pill.urgent`, `.pill.soon`, `.pill.low` | Short status at the end of a row; the warn and crit ones carry an icon. |
| Progress | `<progress class="bar warn" max="100" value="…">` | The numbers are always written next to it. Classes: none (covered), `warn`, `crit`. |
| Readiness | `.ready` | The big "N of D days" card on Home and Plan. |
| Button | `.btn`, `.btn.primary`, `.btn.danger`, `.btn.small`, `.btn.block`, `.btn.big` | One `.primary` per group. Labels are verbs ("Add stock", "Used up"). |
| Chips | `nav.chips > a.chip` (`aria-current` on the chosen one) | Filters as links, so they work without JavaScript. |
| Field | `<label>` wrapping text + `<input>` | Visible label. Placeholder only as an example ("e.g. 1.5"). |
| Empty state | `li.empty` | Says what's true and what to do next. |
| Notice | `.notice` (`role="status"`) | The message after an action, with an optional Undo. |
| Disclosure | `<details>` with a `<summary>` | For editing and for secondary information. |

Helpers: `.muted`, `.small`, `.right`, `.mt`, `.m0`, `.pt0`, `.flex`,
`.between`, `.end`, `.grid2`, `.grid3`, `.stack`, `.vh` (visually hidden,
still read aloud).

### Status vocabulary

The same four words and icons everywhere:

| State | Icon | Colour | Examples |
|---|---|---|---|
| Covered / fine | check | `--accent`, or no colour at all in lists | "Covered", "Nothing needs attention." |
| Soon | clock | `--warn` | "In 5 days", "Low", "6 of 14 days" |
| Expired / missing | exclamation in a circle | `--crit` | "Expired 3 days ago", "Missing 12 L" |
| No date | none | `--ink-2` | "no date" |

## 8. Screens

### Home

1. **Readiness**: "Ready for N of D days" in the biggest type on the page,
   where N is how long the weakest essential lasts (capped at D), a progress
   bar for overall coverage, people in the household, and what isn't covered
   yet. The whole card links to Plan.
2. **Needs attention**: one list in order of urgency: expired, expiring within
   7 days, expiring within 14 days, running low. Each row says what it is and
   when. Expired and urgent food has "Used up" and "Thrown away" right there.
   When nothing needs attention, say so with a check.
3. **Scan to add** and **Scan to use up**, the two everyday actions.
4. **Recent activity**, closed by default.

### Stock

1. Search field, always visible, with a visible button (works without
   JavaScript).
2. Filter chips: All, Food, Supplies, Documents, Things, Running low.
3. Sort as links: by name or by expiry.
4. Rows: name, quantity and unit, next expiry; a pill when it is low or
   expiring. The number of items shown.
5. Adding is the middle tab, so there is no second floating Add button.

### Plan

1. The same readiness card as Home, with the household and the number of days
   to plan for, and the official presets for those days.
2. **Coverage**: one row per target with its status in words ("Covered",
   "Missing 12 L"), the amounts, and a bar. Tap to see what counts towards
   it and to edit the target.
3. "Put the gaps on the shopping list", the main action, right after.
4. Medicine and essentials, suggestions for the household.
5. Tools: printable checklist, power cut.
6. Add a target, closed by default.

## 9. Writing

As in `AGENTS.md`: plain, short, concrete, full sentences, no exclamation
marks. In addition:

- Write numbers with units and "of": "9 of 56 L", "3 of 14 days".
- Say what to do, not what went wrong inside: "Enter a number, like 1.5."
- Every visible text goes through `|t` or `tr()` and into every
  `locales/*.tsv`. German is often 30 % longer: leave room, let text wrap.

## 10. Checks

Automatic, in `cargo test` (`tests/ui.rs`):

- Contrast of every token pair in light and dark against WCAG AA.
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
  (2.5.8) asks for 24 × 24 px; 44 px is the AAA level (2.5.5). Vorrat still
  uses 44 px as its own rule, for gloves, shaking hands and small screens.
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
