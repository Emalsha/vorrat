//! Interface checks from docs/ui-guideline.md: colour contrast of the design
//! tokens in both themes, and template rules the CSP and escaping depend on.

use std::collections::HashMap;

const CSS: &str = include_str!("../static/app.css");
const DESIGN: &str = include_str!("../docs/design/DESIGN.md");
const BRAND: &str = include_str!("../docs/design/brand.json");

/// How the colour names in the Open Design export (`docs/design/DESIGN.md`)
/// map onto the tokens in `static/app.css`.
const DESIGN_TOKENS: &[(&str, &str)] = &[
    ("paper", "bg"),
    ("card", "surface"),
    ("ink", "ink"),
    ("ink-2", "ink-2"),
    ("line", "line"),
    ("signal-blue", "ok"),
    ("signal-yellow", "yellow"),
    ("signal-red", "crit"),
];

/// The `--name: #rrggbb` tokens of a theme: light from the first `:root`
/// rule, dark from the `:root` rule inside `prefers-color-scheme: dark`.
fn tokens(theme: &str) -> HashMap<String, [u8; 3]> {
    let from = match theme {
        "light" => 0,
        _ => CSS
            .find("@media (prefers-color-scheme: dark)")
            .expect("a dark theme in app.css"),
    };
    let block = &CSS[from..];
    let block = &block[block.find(":root {").expect(":root rule")..];
    let block = &block[..block.find('}').expect("end of :root rule")];
    let re = regex::Regex::new(r"--([a-z0-9-]+):\s*#([0-9a-fA-F]{6})\b").unwrap();
    let t: HashMap<String, [u8; 3]> = re
        .captures_iter(block)
        .map(|c| {
            let h = &c[2];
            let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap();
            (c[1].to_string(), [byte(0), byte(2), byte(4)])
        })
        .collect();
    assert!(
        t.len() >= 14,
        "{theme}: only {} colour tokens found",
        t.len()
    );
    t
}

/// WCAG 2.2 relative luminance and contrast ratio.
fn luminance(c: [u8; 3]) -> f64 {
    let ch = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * ch(c[0]) + 0.7152 * ch(c[1]) + 0.0722 * ch(c[2])
}

fn contrast(a: [u8; 3], b: [u8; 3]) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// (foreground, background, minimum ratio). Text needs 4.5:1 (WCAG 1.4.3);
/// field borders, focus rings, progress fills and status shapes need 3:1
/// (1.4.11). Yellow never carries text and is too light on white to be a
/// shape's only edge, so the triangle has an ink edge: `ink` on `bg` and
/// `surface` covers it.
const PAIRS: &[(&str, &str, f64)] = &[
    ("ink", "bg", 4.5),
    ("ink", "surface", 4.5),
    ("ink-2", "bg", 4.5),
    ("ink-2", "surface", 4.5),
    ("accent-ink", "accent", 4.5),
    ("accent-ink", "ink-2", 4.5),
    ("ink", "accent-soft", 4.5),
    ("ok", "bg", 4.5),
    ("ok", "surface", 4.5),
    ("crit", "bg", 4.5),
    ("crit", "surface", 4.5),
    ("banner-ink", "banner", 4.5),
    ("accent", "bg", 3.0),
    ("accent", "surface", 3.0),
    ("field", "surface", 3.0),
    ("field", "bg", 3.0),
    ("focus", "bg", 3.0),
    ("focus", "surface", 3.0),
    ("accent", "track", 3.0),
];

#[test]
fn colour_tokens_meet_wcag_aa_in_both_themes() {
    let mut fails = Vec::new();
    for theme in ["light", "dark"] {
        let t = tokens(theme);
        for (fg, bg, min) in PAIRS {
            let (Some(f), Some(b)) = (t.get(*fg), t.get(*bg)) else {
                fails.push(format!("{theme}: --{fg} or --{bg} is missing"));
                continue;
            };
            let r = contrast(*f, *b);
            if r < *min {
                fails.push(format!(
                    "{theme}: --{fg} on --{bg} is {r:.2}:1, needs {min}:1"
                ));
            }
        }
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

#[test]
fn both_themes_define_the_same_colours() {
    let (mut l, mut d): (Vec<_>, Vec<_>) = (
        tokens("light").into_keys().collect(),
        tokens("dark").into_keys().collect(),
    );
    l.sort();
    d.sort();
    assert_eq!(l, d);
}

/// The `name: "#rrggbb"` colours in the front matter of `DESIGN.md`.
fn design_colours() -> HashMap<String, [u8; 3]> {
    let re = regex::Regex::new(r##"(?m)^\s+([a-z0-9-]+):\s*"#([0-9a-fA-F]{6})""##).unwrap();
    let front = &DESIGN[..DESIGN.find("\n---\n\n#").unwrap_or(DESIGN.len())];
    re.captures_iter(front)
        .map(|c| {
            let h = &c[2];
            let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap();
            (c[1].to_string(), [byte(0), byte(2), byte(4)])
        })
        .collect()
}

#[test]
fn design_system_colours_match_the_tokens() {
    let (design, css) = (design_colours(), tokens("light"));
    let mut fails = Vec::new();
    for (name, token) in DESIGN_TOKENS {
        match (design.get(*name), css.get(*token)) {
            (Some(d), Some(c)) if d == c => {}
            (d, c) => fails.push(format!("DESIGN.md {name} is {d:?}, --{token} is {c:?}")),
        }
    }
    let extra: Vec<_> = design
        .keys()
        .filter(|k| !DESIGN_TOKENS.iter().any(|(n, _)| n == k))
        .collect();
    assert!(
        extra.is_empty(),
        "DESIGN.md colours without a token: {extra:?}"
    );
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

/// `brand.json` is the Open Design project file: its palette must say the
/// same as `DESIGN.md`, and its seed must use the same ink, signal colours,
/// square corners and control height, so the next export doesn't bring back
/// an older design.
#[test]
fn open_design_project_matches_design_md() {
    let brand: serde_json::Value = serde_json::from_str(BRAND).expect("brand.json is JSON");
    let design = design_colours();
    let hex = |h: &str| {
        let h = h.trim_start_matches('#');
        let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap();
        [byte(0), byte(2), byte(4)]
    };
    let mut fails = Vec::new();
    let colours = brand["colors"].as_array().expect("brand.json colors");
    for c in colours {
        let name = c["name"].as_str().unwrap().to_lowercase().replace(' ', "-");
        let value = hex(c["hex"].as_str().unwrap());
        match design.get(&name) {
            Some(d) if *d == value => {}
            d => fails.push(format!(
                "brand.json {name} is {value:?}, DESIGN.md has {d:?}"
            )),
        }
    }
    if colours.len() != design.len() {
        fails.push(format!(
            "brand.json has {} colours, DESIGN.md {}",
            colours.len(),
            design.len()
        ));
    }
    let seed = &brand["seed"];
    for (key, name) in [
        ("colorPrimary", "ink"),
        ("colorTextBase", "ink"),
        ("colorSuccess", "signal-blue"),
        ("colorWarning", "signal-yellow"),
        ("colorError", "signal-red"),
    ] {
        let v = seed[key].as_str().map(hex);
        if v.as_ref() != design.get(name) {
            fails.push(format!(
                "brand.json seed.{key} is {v:?}, DESIGN.md {name} differs"
            ));
        }
    }
    if seed["borderRadius"] != 0 {
        fails.push("brand.json seed.borderRadius must be 0".into());
    }
    if seed["controlHeight"] != 48 {
        fails.push("brand.json seed.controlHeight must be 48".into());
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

#[test]
fn contrast_formula_matches_wcag() {
    assert!((contrast([0, 0, 0], [255, 255, 255]) - 21.0).abs() < 0.01);
    assert!((contrast([0x76; 3], [255; 3]) - 4.54).abs() < 0.01);
}

/// Every template under `templates/`, with its path and text.
fn templates() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut dirs = vec![std::path::PathBuf::from("templates")];
    while let Some(d) = dirs.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                dirs.push(p);
            } else {
                let text = std::fs::read_to_string(&p).unwrap();
                out.push((p.display().to_string(), text));
            }
        }
    }
    out.sort();
    out
}

/// The CSP is `style-src 'self'`: inline styles would silently do nothing.
#[test]
fn templates_have_no_inline_styles_or_scripts() {
    let handler = regex::Regex::new(r"\son[a-z]+=").unwrap();
    for (name, text) in templates() {
        assert!(
            !text.contains("style=\""),
            "{name}: use a class, not style=\"…\""
        );
        assert!(
            !text.contains("<style"),
            "{name}: styles belong in static/app.css"
        );
        assert!(
            !text.contains("<script>"),
            "{name}: scripts belong in static/"
        );
        assert!(!handler.is_match(&text), "{name}: no inline event handlers");
    }
}

/// Askama escapes by default; `|safe` would let typed text through as HTML.
#[test]
fn templates_never_mark_text_safe() {
    for (name, text) in templates() {
        assert!(!text.contains("|safe"), "{name} uses |safe");
    }
}
