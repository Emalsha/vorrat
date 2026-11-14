# Vorrat

---
name: "Vorrat"
category: Brands
surface: web
colors:
  paper: "#eeede9"
  card: "#ffffff"
  ink: "#141414"
  ink-2: "#4d4c48"
  line: "#d9d8d3"
  signal-blue: "#1f4fa0"
  signal-yellow: "#f2b705"
  signal-red: "#c0261d"
---

# Vorrat

> Category: Brands

> Surface: web

*Know if your household is ready for a crisis, and stay ready.*

Vorrat (German for "supplies") is a private, self-hosted app for household emergency preparedness. It tracks real stock batch by batch with expiry dates, plans supplies for N people over D days from official civil-protection advice, and runs offline on a small machine such as a Raspberry Pi. The interface follows the Bauhaus idea that form follows the task: ink on paper, a grid of rules instead of boxes, square corners, and three signal colours that each mean one thing. Server-rendered, one stylesheet, one bundled font, WCAG 2.2 AA.

## Principles

The design follows the Bauhaus idea that form follows the task. Every form on a screen has one job, and anything that has none is left out. These principles decide cases the rules below don't cover.

- **Form follows the task.** Each screen answers one question. What doesn't help answer it moves into a disclosure or onto another page.
- **Shape carries meaning.** Kandinsky's 1923 survey at the Bauhaus paired the circle with blue, the triangle with yellow and the square with red. Vorrat uses that pairing for status, so status reads by shape alone and colour is never the only cue (WCAG 1.4.1).
- **Rules, not boxes.** Sections are separated by 2px Ink rules on a grid. No rounded cards, shadows or soft fills behind every row.
- **Count in units.** What is counted in days is drawn as days. Numbers carry their unit and sit where the eye checks them.
- **Black is the action.** The one solid button is Ink. Colour is kept for state, so anything coloured on screen means "look here", and a calm screen is ink on paper.
- **Honest about limits.** Bauhaus lowercase-only type and geometric body text are not used: both slow reading, and German capitalises nouns. Jost is for headings and numbers only; body text stays in the system face.

### Why these choices

- Green is not used. It used to mean both "press this" and "covered", so the most common thing on screen and the calmest state shared a colour.
- The focus ring is Signal Blue rather than Ink, so it stands out from the 2px Ink edges of buttons and fields.
- Yellow is too light for text on white (1.8:1). It only fills the triangle, and the triangle's Ink edge gives the shape its contrast.
- Jost is bundled into the binary (about 23 KB) and served by Vorrat itself, so the app stays offline and needs no outside host. If it is missing, Futura or the system face takes over and nothing breaks.

## Color Palette

| Role | Name | Hex | Usage |
| --- | --- | --- | --- |
| background | Paper | `#eeede9` | Page background (--bg). A neutral, very slightly warm grey. Dark: #121212. |
| surface | Card | `#ffffff` | Header, navigation, inputs and hovered rows (--surface). Sections are not boxed: they sit on Paper under a 2px Ink rule. Dark: #1d1d1c. |
| foreground | Ink | `#141414` | Text, headings, rules, control edges, the primary button and the day squares (--ink, also --accent). 15.7:1 on Paper, 18.4:1 on Card. Text on Ink is #ffffff. Dark: #f0efea with #121212 on it. |
| muted | Ink 2 | `#4d4c48` | Secondary text and metadata (--ink-2). 7.3:1 on Paper, 8.6:1 on Card. Also the hover fill of the primary button. Dark: #b3b1ab. |
| border | Line | `#d9d8d3` | Hairlines between rows and the empty part of a bar (--line, --track). Never the only edge of a control; fields use Field #6b6a66 (dark #8a8984, 4.8:1 or better). Dark: #343432, track #3a3a38. |
| accent | Signal Blue | `#1f4fa0` | Covered: the circle and its word (--ok), and the 3px focus ring (--focus). 7.8:1 on Card. Dark: #86a8ee. |
| accent-secondary | Signal Yellow | `#f2b705` | Soon and low: only as the fill of the triangle, which has a 1.6px Ink edge (--yellow). Never text: 1.8:1 on Card. The same in both themes. |
| accent-tertiary | Signal Red | `#c0261d` | Expired, missing, destructive: the square, its word, and the edge and text of the danger button (--crit). 5.9:1 on Card, 5.1:1 on Paper. Dark: #ff7b6e. |

## Typography
- **Display:** Jost — weights 400 to 700 — fallbacks: Futura, Century Gothic, system-ui, sans-serif (One variable woff2, subset to Latin and Latin Extended, about 23 KB, SIL Open Font License, bundled in the binary and served from /static/jost.woff2. Never loaded from the internet. Used for page titles, section labels, the big readiness number, counts and the brand name. Page title 1.375rem (1.75rem on desktop) at 600; the readiness number 4rem at 600 with its unit at 1.375rem 400 in Ink 2. Section labels 0.75rem, 600, uppercase, letter-spacing .14em, in Ink.)
- **Body:** system-ui — weights 400, 500, 600, 650 — fallbacks: -apple-system, BlinkMacSystemFont, Segoe UI, Roboto, sans-serif (Body text stays in the platform face because it reads best at small sizes on cheap screens. 1rem/1.45. Small text 0.875rem; nothing below 0.75rem. Inputs are 16px so iOS does not zoom. Labels and buttons 600.)
- **Mono:** ui-monospace — weights 400 — fallbacks: SFMono-Regular, Menlo, Consolas, monospace (Barcodes and codes only.)

Bauhaus lowercase-only type is not used: German capitalises nouns, and mixed case reads faster.

## Voice & Tone

- **Adjectives:** calm, plain, concrete, trustworthy, private, unalarmed
- **Tone:** A calm instrument panel, not an app that wants attention. Short, full sentences in active voice, no jargon, no exclamation marks. It informs and never frightens: no sirens, countdowns or red banners. It says what is true and what to do next, in words people read on a bad day, tired and one-handed. Numbers always carry units and "of" ("9 of 56 L", "3 of 14 days"); dates are human ("3 days ago", "12 Mar 2027"). Errors say what to fix, not what broke inside ("Enter a number, like 1.5.").

### Messaging pillars
- Emergency plan first: targets come from official civil-protection advice (BBK) and adapt to household size and days.
- Tracks real stock: batch by batch with expiry dates; use-up takes the batch that expires first.
- Private by design: one binary, one SQLite file on your own machine. No account, no telemetry, no cloud.
- Small and offline: runs on a Raspberry Pi, works without internet and without JavaScript.
- Earned engagement: useful every day (expiry, shopping), never streaks, badges or nagging.

### Vocabulary
- **Use:** Covered, Low, Expired 3 days ago, In 5 days, Missing 12 L, Ready for N of D days, Use first, Running low, Nothing needs attention., Add stock, Used up, Thrown away, Scan to add, Undo, household, stock, essentials
- **Avoid:** exclamation marks, Oops / Uh-oh, urgent alarmist wording ("Danger!", "Act now"), streaks, badges, rewards, jargon such as SKU or inventory unit, error codes in UI text, invented official numbers

## Imagery

- **Style:** Functional, not decorative: user photos of their own products and documents, small inline SVG line icons, the three status shapes, and day squares.
- **Subjects:** the household's own product photos (thumbnails from a barcode lookup or a phone upload), scanned documents, 2px-stroke line icons: home, stock grid, plus, shield-check, cart, medical case, people, location pin, gear
- **Treatment:** Product thumbnails are 42px squares (64–88px on forms and item pages), square corners, object-fit cover. Without a photo there is no thumbnail. Icons are inline SVG, 24×24 viewBox, stroke currentColor, stroke-width 2. Status shapes are filled SVG at 0.8em: circle in Signal Blue, triangle in Signal Yellow with an Ink edge, square in Signal Red. The logo is the circle, triangle and square on Paper above an Ink band. The camera scanner is the only black surface: black with a white 3px frame.
- **Avoid:** stock photography, hero illustrations, hand-drawn people, emoji as icons, icon fonts or icon CDNs, gradients, drop shadows, rounded corners, alarm imagery such as sirens or warning tape

## Layout

- **Radius:** 0 everywhere. The circle status shape is the only round form.
- **Border weight:** 2px Ink rules above sections, under the header and above the tab bar; 2px Ink on buttons and chips; 2px Field on inputs and tick boxes; 1px Line between rows; 3px Signal Blue focus ring.
- **Spacing:** 4px base: 4, 8, 12, 16, 24, 32 px (--s1…--s6), nothing in between

### Posture rules
- Every screen answers, in order: Are we all right? What needs doing? How do I do it? The most important number is first and biggest.
- Readiness is counted in days and drawn as days: one square per planned day (up to 31), filled in Ink for each whole day the weakest essential lasts. Over 31 days, a square-ended bar.
- Phone-first, one column at most 720px wide with 16px gutters and a fixed five-place bottom navigation (Home, Stock, Add, Plan, Shopping). Add is a solid Ink cell in the same grid, not a raised button. The current place has a 4px Ink bar above it and bold text. From 64em a 248px sidebar replaces it (Add stock as the one solid button; current item = Paper fill, bold, 4px Ink bar on the left); from 75em pages use .cols at 3:2.
- Rules, not boxes: a section is a label, a 2px Ink rule and rows separated by 1px Line hairlines, on Paper. No cards, no shadows.
- Every tap target is at least 48px tall (40px for small buttons and chips, rows 56px). One solid button per group; other buttons are transparent with a 2px Ink edge.
- Home groups work by what you do with it: Expired (one Thrown away button per row), Use first (one Used up button per row), Running low. The other choice lives on the item page.
- Plan lists the essentials least covered first, each with its day squares, its amounts and its status.
- Colour means 'look here': the calm state is ink on paper. Status always pairs a shape, a word and a colour: circle = Covered, triangle = Soon/Low, square = Expired/Missing. Bars and day squares are always Ink; the colour sits in the status word beside them.
- Components: .btn (transparent, 2px Ink edge, square, 48px, weight 600; .primary Ink fill with Paper text; .danger Signal Red edge and text); field = visible 0.875rem label above a 48px input with a 2px Field border and 16px text; .chip = 40px square, 2px Ink edge, selected Ink fill; .pill = 0.875rem status shape plus word, no fill; .count = the quantity at the end of a stock row in Jost with tabular figures; .notice = Card with a 2px Ink edge, role=status, optional Undo; demo and offline notes are a thin Ink band.
- Type scale: readiness number 4rem/600/line-height .95 with its unit in 1.375rem Ink 2; page title 1.375rem (1.75rem desktop); section labels 0.75rem uppercase +.14em in Ink; body 1rem/1.45; small 0.875rem; nothing below 0.75rem.
- States: hover thickens link underlines, fills secondary buttons and rows with Card or a light grey, turns the solid button Ink 2. Links are Ink and always underlined. Focus is a 3px Signal Blue outline at 2px offset, never removed. Current item uses aria-current plus a non-colour cue.
- No motion: nothing animates, fades or moves on its own. Notices stay until the next page so Undo can't vanish. Honour prefers-reduced-motion and forced-colors by keeping borders.
- Secondary information (files, barcodes, prices, history) lives in native <details> disclosures so the first screen shows only daily essentials.
- Don't: fonts from the internet, shadows, gradients, rounded corners, animation, fading toasts, red full-screen banners, several solid buttons for one action, inline styles, emoji icons, exclamation marks, yellow text.
