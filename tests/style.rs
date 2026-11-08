//! The comment rule from `AGENTS.md`: comments only as a doc comment at the
//! top of an item (function, struct, enum, trait, impl, const, module), never
//! inside bodies, attributes, struct or enum definitions, CSS rules or
//! templates, and never with pull request or issue numbers.

use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// A pull request or issue named by number, with or without a hash sign.
static TICKET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(#\d+|\b(pr|pull request|issue)\s*#?\d+)").unwrap());

/// A comment found in a file: its line, its text and its brace depth.
struct Comment {
    line: usize,
    text: String,
    depth: i32,
}

/// Files with the given extension under `dir`, skipping `static/vendor`.
fn files(dir: &str, ext: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::from(dir)];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.ends_with("vendor") {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == ext) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Finds `//` and `/* */` comments outside string literals. `quotes` lists
/// the characters that open a string; Rust raw strings and lifetimes are
/// handled when `rust` is set.
fn comments(src: &str, quotes: &[char], rust: bool) -> Vec<Comment> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line, mut depth) = (0, 1, 0);
    while i < c.len() {
        let ch = c[i];
        let next = c.get(i + 1).copied();
        if ch == '\n' {
            line += 1;
            i += 1;
        } else if ch == '/' && next == Some('/') {
            let start = i;
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            out.push(Comment {
                line,
                text: c[start..i].iter().collect(),
                depth,
            });
        } else if ch == '/' && next == Some('*') {
            let (start, first) = (i, line);
            i += 2;
            while i + 1 < c.len() && !(c[i] == '*' && c[i + 1] == '/') {
                if c[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            i += 2;
            out.push(Comment {
                line: first,
                text: c[start..i.min(c.len())].iter().collect(),
                depth,
            });
        } else if rust && ch == 'r' && raw_start(&c, i) {
            let mut hashes = 0;
            i += 1;
            while c[i] == '#' {
                hashes += 1;
                i += 1;
            }
            i += 1;
            loop {
                if c[i] == '\n' {
                    line += 1;
                }
                if c[i] == '"' && (1..=hashes).all(|h| c.get(i + h) == Some(&'#')) {
                    i += 1 + hashes;
                    break;
                }
                i += 1;
            }
        } else if rust && ch == '\'' {
            if next == Some('\\') {
                i += 2;
                while c[i] != '\'' {
                    i += 1;
                }
                i += 1;
            } else if c.get(i + 2) == Some(&'\'') {
                i += 3;
            } else {
                i += 1;
            }
        } else if quotes.contains(&ch) {
            i += 1;
            while i < c.len() && c[i] != ch {
                if c[i] == '\\' {
                    i += 1;
                }
                if c[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            i += 1;
        } else {
            match ch {
                '{' | '(' | '[' => depth += 1,
                '}' | ')' | ']' => depth -= 1,
                _ => {}
            }
            i += 1;
        }
    }
    out
}

/// Whether a Rust raw string (`r"`, `r#"`, `br"`) starts at `i`.
fn raw_start(c: &[char], i: usize) -> bool {
    let before = if i > 0 && c[i - 1] == 'b' { i - 1 } else { i };
    if before > 0 && (c[before - 1].is_alphanumeric() || c[before - 1] == '_') {
        return false;
    }
    let mut j = i + 1;
    while c.get(j) == Some(&'#') {
        j += 1;
    }
    c.get(j) == Some(&'"')
}

/// Whether a doc comment on `line` documents an item: the next line that is
/// not a doc comment or an attribute declares a function, type, module,
/// constant or impl block.
fn documents_item(lines: &[&str], line: usize) -> bool {
    let next = lines[line..].iter().map(|l| l.trim()).find(|l| {
        !l.is_empty() && !l.starts_with("///") && !l.starts_with("#[") && !l.starts_with(")]")
    });
    let Some(mut l) = next else { return false };
    for prefix in ["pub(crate) ", "pub(super) ", "pub ", "async ", "unsafe "] {
        l = l.strip_prefix(prefix).unwrap_or(l);
    }
    [
        "fn ",
        "struct ",
        "enum ",
        "mod ",
        "const ",
        "static ",
        "trait ",
        "impl",
        "type ",
        "macro_rules!",
    ]
    .iter()
    .any(|k| l.starts_with(k))
}

/// Whether the comment names a pull request or an issue by number.
fn names_ticket(text: &str) -> bool {
    TICKET.is_match(text)
}

fn report(problems: &mut Vec<String>, path: &Path, line: usize, why: &str) {
    problems.push(format!("{}:{line}: {why}", path.display()));
}

/// Rust: only `///` on an item and `//!` at the top of the file.
fn check_rust(path: &Path, problems: &mut Vec<String>) {
    let src = fs::read_to_string(path).unwrap();
    let lines: Vec<&str> = src.lines().collect();
    let header = lines
        .iter()
        .take_while(|l| l.starts_with("//!") || l.trim().is_empty())
        .count();
    for c in comments(&src, &['"'], true) {
        if names_ticket(&c.text) {
            report(problems, path, c.line, "pull request or issue number");
        }
        let doc = c.text.starts_with("///") && !c.text.starts_with("////");
        let inner = c.text.starts_with("//!");
        let own_line = lines[c.line - 1].trim_start().starts_with("//");
        if inner && c.line <= header {
            continue;
        }
        if doc && own_line && documents_item(&lines, c.line) {
            continue;
        }
        report(
            problems,
            path,
            c.line,
            "comment outside an item's doc comment",
        );
    }
}

/// CSS and JavaScript: comments only on their own line at the top level, or
/// right above a JavaScript function, never inside a rule, function or call.
fn check_top_level(path: &Path, quotes: &[char], problems: &mut Vec<String>) {
    let src = fs::read_to_string(path).unwrap();
    let lines: Vec<&str> = src.lines().collect();
    for c in comments(&src, quotes, false) {
        if names_ticket(&c.text) {
            report(problems, path, c.line, "pull request or issue number");
        }
        let own_line = lines[c.line - 1].trim_start().starts_with("/");
        let above_function = lines[c.line..]
            .iter()
            .map(|l| l.trim())
            .find(|l| !l.starts_with("//") && !l.starts_with('*'))
            .is_some_and(|l| l.starts_with("function "));
        if !own_line || (c.depth != 0 && !above_function) {
            report(problems, path, c.line, "comment inside a rule or function");
        }
    }
}

/// Templates: no HTML or Askama comments at all.
fn check_template(path: &Path, problems: &mut Vec<String>) {
    let src = fs::read_to_string(path).unwrap();
    for (n, l) in src.lines().enumerate() {
        if l.contains("<!--") || l.contains("{#") {
            report(problems, path, n + 1, "comment in a template");
        }
    }
}

#[test]
fn comments_follow_the_rule() {
    let mut problems = Vec::new();
    for dir in ["src", "tests"] {
        for p in files(dir, "rs") {
            check_rust(&p, &mut problems);
        }
    }
    for p in files("static", "js") {
        check_top_level(&p, &['"', '\'', '`'], &mut problems);
    }
    for p in files("static", "css") {
        check_top_level(&p, &['"', '\''], &mut problems);
    }
    for p in files("templates", "html") {
        check_template(&p, &mut problems);
    }
    assert!(
        problems.is_empty(),
        "comments break the rule in AGENTS.md:\n{}",
        problems.join("\n")
    );
}

#[test]
fn the_checker_sees_what_it_should() {
    let rust = "/// Doc.\nfn a() {\n    let s = \"// not a comment\";\n    // inside\n}\n";
    let found = comments(rust, &['"'], true);
    assert_eq!(found.len(), 2);
    assert_eq!(found[1].line, 4);
    let lines: Vec<&str> = rust.lines().collect();
    assert!(documents_item(&lines, 1));
    let field = "struct A {\n    /// A field.\n    pub b: i32,\n}\n";
    let lines: Vec<&str> = field.lines().collect();
    assert!(!documents_item(&lines, 2));
    assert!(names_ticket("see #12"));
    assert!(names_ticket("from PR 7"));
    assert!(!names_ticket("v4 schema"));
    let css = "/* top */\na { color: red; /* in */ }\n";
    let found = comments(css, &['"'], false);
    assert_eq!((found[0].depth, found[1].depth), (0, 1));
}
