# Vorrat

---
name: "Vorrat"
category: Brands
surface: web
colors:
  paper: "#f5f4f0"
  card: "#ffffff"
  ink: "#1d1c1a"
  ink-2: "#5c5851"
  line: "#e4e1da"
  forest: "#2f6f4f"
  focus-blue: "#1f5fa8"
---

# Vorrat

> Category: Brands

> Surface: web

*Know if your household is ready for a crisis, and stay ready.*

Vorrat (German for "supplies") is a private, self-hosted app for household emergency preparedness. It tracks real stock batch by batch with expiry dates, plans supplies for N people over D days from official civil-protection advice, and runs offline on a small machine such as a Raspberry Pi. The interface is a calm instrument panel: server-rendered, system fonts, one stylesheet, WCAG 2.2 AA.

## Color Palette

| Role | Name | Hex | Usage |
| --- | --- | --- | --- |
| background | Paper | `#f5f4f0` | Page background (--bg). A warm, very low-chroma off-white that keeps the calm state quiet. Also the sticky header and the row hover fill. Measured from static/app.css :root. Dark: #121311. |
| surface | Card | `#ffffff` | Cards, lists, inputs and the navigation bar (--surface). White surfaces with a 1px border and no shadow. Dark: #1c1d1b. |
| foreground | Ink | `#1d1c1a` | Body text and headings (--ink). 15.5:1 on Paper, 17.0:1 on Card. Dark: #ecebe7. |
| muted | Ink 2 | `#5c5851` | Secondary text, section headings, metadata, inactive tabs (--ink-2). 6.4:1 on Paper, 7.1:1 on Card. Dark: #a8a49c. |
| border | Line | `#e4e1da` | Decorative dividers and card edges only (--line). Never the only edge of a control; fields use Field. Control edges use Field #8c877e (dark #77736b, 3.6:1); progress tracks use #d6d2ca (dark #3a3b38). Dark: #2c2d2a. |
| accent | Forest | `#2f6f4f` | Primary buttons, links, current tab, the covered state, progress fill (--accent). 6.0:1 on Card. Matches the icon tile and theme-color. Use only where action is needed; the calm state stays uncolored. Text on Forest is #ffffff; the soft fill for secondary buttons and notices is Forest Soft #e8f1ec (Forest on it 5.2:1). Dark: accent #6cc095 with ink #0d1a13 and soft #1d2b23. |
| accent-secondary | Focus Blue | `#1f5fa8` | The 3px :focus-visible ring and information notes (--focus). 6.4:1 on Card. Not for buttons or decoration. Dark: #8ab4ff. |

## Typography
- **Display:** system-ui — weights 600, 650, 750 — fallbacks: -apple-system, BlinkMacSystemFont, Segoe UI, Roboto, sans-serif (Measured. Vorrat ships no web fonts by rule (offline, no CDNs, Raspberry Pi). Headings use the platform UI face: SF on Apple, Segoe UI on Windows, Roboto on Android. Page title 1.25rem (1.5rem on desktop) at 600–700; the readiness number is 2.25rem at 750 with line-height 1.1. Section headings (h2) are 0.75rem, uppercase, letter-spacing .06em, in Ink 2.)
- **Body:** system-ui — weights 400, 500, 600, 650 — fallbacks: -apple-system, BlinkMacSystemFont, Segoe UI, Roboto, sans-serif (Measured: body is 1rem/1.45. The real stack order in app.css is -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, Roboto, sans-serif. Small text 0.875rem; nothing below 0.75rem, and 0.75rem only for tab labels. Inputs are 16px so iOS does not zoom. Labels and buttons 600–650.)
- **Mono:** ui-monospace — weights 400 — fallbacks: SFMono-Regular, Menlo, Consolas, monospace (Inferred: app.css only sets code at .9em with the browser default monospace. Use for barcodes and codes only.)

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
- **Use:** Covered, Low, Expired 3 days ago, In 5 days, Missing 12 L, Ready for N of D days, Needs attention, Nothing needs attention., Add stock, Used up, Thrown away, Scan to add, Undo, household, stock, essentials
- **Avoid:** exclamation marks, Oops / Uh-oh, urgent alarmist wording ("Danger!", "Act now"), streaks, badges, rewards, jargon such as SKU or inventory unit, error codes in UI text, invented official numbers

## Imagery

- **Style:** Functional, not decorative: user photos of their own products and documents, small inline SVG line icons, and native progress bars.
- **Subjects:** the household's own product photos (thumbnails from a barcode lookup or a phone upload), scanned documents such as a passport or insurance, 2px-stroke line icons: home, stock grid, plus, shield-check, cart, medical case, people, location pin, gear
- **Treatment:** Product thumbnails are 42px squares (64–88px on forms and item pages) with a 10px radius, object-fit cover and a Forest Soft placeholder that shows the initial letter. Icons are inline SVG, 24×24 viewBox, stroke currentColor, stroke-width 2 (2.5 for status icons), round caps and joins, no fills. The camera scanner is the only dark surface: black with a white 3px frame.
- **Avoid:** stock photography, hero illustrations, hand-drawn people, emoji as icons, icon fonts or icon CDNs, decorative gradients, drop shadows, alarm imagery such as sirens or warning tape

## Layout

- **Radius:** 14px cards, 10px buttons and fields, 999px pills and chips, 6px checkboxes
- **Border weight:** 1px (Line on cards, Field on controls); 2px tick boxes; 3px focus ring
- **Spacing:** 4px base: 4, 8, 12, 16, 24, 32 px (--s1…--s6), nothing in between

### Posture rules
- Every screen answers, in order: Are we all right? What needs doing? How do I do it? The most important number is first and biggest.
- Phone-first, one column at most 720px wide with 16px gutters and a fixed five-place bottom navigation (Home, Stock, Add, Plan, Shopping), with Add as a raised round 48px button in the middle. From 64em a 248px sidebar replaces it (Add stock as the one primary button; current item = Forest Soft fill, Forest text, 4px left bar); from 75em pages use .cols at 3:2.
- Flat surfaces: white cards with a 1px Line border and 14px radius on Paper, no shadows. Depth comes from the border, not elevation.
- Every tap target is at least 44px tall (40px for small buttons and chips, rows 56px). One primary button per group; secondary actions are Forest Soft #e8f1ec with Forest text.
- Status palette (measured; part of the brand palette even though outside the seven core roles): Amber #8a5a00 on Amber Soft #fff3d6 for soon, low and partly covered (dark #ffcc66 on #352a14); Brick #b3261e on Brick Soft #fbe9e7 for expired, missing and destructive (dark #ff8a80 on #3a1d1b); Slate #2f4f6f with white text for demo and offline notes.
- Colour means 'look here': the calm state is uncolored. Status always pairs a colour with an icon and a word: check = Covered, clock = Soon/Low, exclamation in a circle = Expired/Missing.
- Components: .btn (Forest Soft, 10px radius, 44px, weight 650; .primary Forest/white; .danger Brick Soft/Brick); field = visible 0.875rem label above a 44px input with a 1px Field border and 16px text; .chip = 40px pill, selected Forest fill; .pill = 0.875rem bold 999px status tag; progress = 10px rounded bar on Track, numbers always beside it; .notice = Forest Soft with 1px Forest border, role=status, optional Undo.
- Type scale: readiness number 2.25rem/750/line-height 1.1 with its unit in 1.25rem Ink 2; page title 1.25rem (1.5rem desktop); section labels 0.75rem uppercase +.06em in Ink 2; body 1rem/1.45; small 0.875rem; nothing below 0.75rem.
- States: hover underlines links, darkens buttons 3%, fills rows with Paper; never fade text to grey. Focus is a 3px Focus Blue outline at 2px offset, never removed. Current item uses aria-current plus a non-colour cue.
- No motion: nothing animates, fades or moves on its own. Notices stay until the next page so Undo can't vanish. Honour prefers-reduced-motion and forced-colors by keeping borders.
- Secondary information (files, barcodes, prices, history) lives in native <details> disclosures so the first screen shows only daily essentials.
- Don't: web fonts or Inter, shadows, gradients, animation, fading toasts, red full-screen banners, several solid buttons for one action, inline styles, emoji icons, exclamation marks.
