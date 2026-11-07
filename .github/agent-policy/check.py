#!/usr/bin/env python3
"""Deterministic policy check for agent-written Vorrat patches. No AI.

    python3 check.py <patch-file>     (or the diff on stdin)

Prints {"verdict": "clean" | "flagged" | "owner", "findings": [...]} and exits
0, 1 or 2. "flagged" patches need a maintainer's careful review (label
agent-strict-review); "owner" patches touch paths only Emalsha changes and
always fail. CI runs the copy from the base branch, so a patch can't loosen
its own check. Adapted from anomalyco/hex's .github/agent-policy/check.ts.
"""
import json
import re
import sys

OWNER_PATHS = [
    (r"^\.github/", "CI, workflows or ownership config"),
    (r"(^|/)CODEOWNERS$", "code ownership"),
    (r"(^|/)(AGENTS|CLAUDE)\.md$", "agent instructions"),
    (r"(^|/)LICENSE$", "licence"),
]

FLAGGED_PATHS = [
    (r"(^|/)Cargo\.toml$", "Cargo manifest (dependencies)"),
    (r"(^|/)Cargo\.lock$", "Cargo.lock"),
    (r"(^|/)build\.rs$", "build script"),
    (r"(^|/)\.cargo/", "cargo config"),
    (r"(^|/)Dockerfile$|(^|/)compose[^/]*\.ya?ml$|(^|/)deploy/", "deployment"),
    (r"(^|/)src/auth\.rs$", "sign-in and sessions"),
    (r"(^|/)src/lookup\.rs$", "outbound network calls"),
    (r"(^|/)src/notify\.rs$", "outbound network calls"),
    (r"(^|/)src/backup\.rs$", "backups"),
    (r"(^|/)static/sw\.js$", "service worker (offline cache)"),
    (r"(^|/)static/vendor/", "vendored third-party code"),
    (r"(^|/)tests/fixtures/", "test fixtures"),
]

LINE_RULES = [
    (r"\bhttps?://|\bwss?://", "new URL"),
    (r"\bCommand::new\b|\bstd::process\b|\bprocess::Command\b|\bchild_process\b", "process spawning"),
    (r"\breqwest\b|\bureq\b|\bhyper::|\bTcpStream\b|\bUdpSocket\b|\bstd::net\b|\bfetch\(|\bXMLHttpRequest\b|\bWebSocket\b|\bsendBeacon\b", "network API"),
    (r"\bunsafe\b", "unsafe code"),
    (r"\|\s*safe\b", "unescaped template output"),
    (r"unsafe-inline|unsafe-eval|content-security-policy", "content security policy"),
    (r"<script(?![^>]*\bsrc=)[^>]*>", "inline script"),
    (r"\bon(click|load|error|submit|change|input)\s*=", "inline event handler"),
    (r"CREATE TABLE|ALTER TABLE|DROP TABLE|user_version", "database schema"),
    (r"[A-Za-z0-9+/]{100,}={0,2}|\b[0-9a-fA-F]{64,}\b|(\\x[0-9a-fA-F]{2}){16,}", "encoded blob"),
    (r"[​-‏‪-‮⁠-⁤⁦-⁩﻿­]", "bidi or zero-width character"),
    (r"\b(env::var|getenv|process\.env)\b.*\b(TOKEN|SECRET|KEY|PASSWORD|CREDENTIAL)", "secret environment read"),
]

MAX_CHANGED_LINES = 600
MAX_FILES = 25


def check(diff: str) -> dict:
    findings = []

    def add(tier, rule, file, line=None):
        if not any(f["rule"] == rule and f["file"] == file for f in findings):
            f = {"tier": tier, "rule": rule, "file": file}
            if line is not None:
                f["line"] = line
            findings.append(f)

    files = set()
    file, new_line, changed = "", 0, 0

    def classify(path):
        if not path or path == "/dev/null":
            return
        files.add(path)
        for rx, rule in OWNER_PATHS:
            if re.search(rx, path):
                add("owner", rule, path)
        for rx, rule in FLAGGED_PATHS:
            if re.search(rx, path):
                add("flagged", rule, path)

    for raw in diff.split("\n"):
        if raw.startswith("diff --git "):
            m = re.match(r"^diff --git a/(.+) b/(.+)$", raw)
            file = m.group(2) if m else raw
            classify(m.group(1) if m else "")
            classify(file)
            continue
        if raw.startswith("rename from "):
            classify(raw[12:])
        if raw.startswith("rename to "):
            file = raw[10:]
            classify(file)
        if re.match(r"^(new|deleted) file mode 120000|^new mode 120000", raw):
            add("flagged", "symlink", file)
        if re.match(r"^(new file mode|new mode) 100755", raw):
            add("flagged", "executable file", file)
        if re.match(r"^Binary files |^GIT binary patch", raw):
            add("flagged", "binary file", file)
        if raw.startswith("+++ ") or raw.startswith("--- "):
            p = re.sub(r"^[ab]/", "", raw[4:])
            if p != "/dev/null":
                classify(p)
            continue
        hunk = re.match(r"^@@ -\d+(?:,\d+)? \+(\d+)", raw)
        if hunk:
            new_line = int(hunk.group(1))
            continue
        if raw.startswith("+"):
            changed += 1
            text = raw[1:]
            for rx, rule in LINE_RULES:
                if re.search(rx, text, re.IGNORECASE if rule in ("new URL", "database schema") else 0):
                    add("flagged", rule, file, new_line)
            if re.search(r"(^|/)Cargo\.toml$", file) and re.match(r"^\s*[A-Za-z0-9_-]+\s*=\s*[\"{]", text):
                add("flagged", "dependency added or changed", file, new_line)
            new_line += 1
        elif raw.startswith("-"):
            changed += 1
        elif raw.startswith(" "):
            new_line += 1

    if changed > MAX_CHANGED_LINES:
        add("flagged", f"size: {changed} changed lines > {MAX_CHANGED_LINES}", "*")
    if len(files) > MAX_FILES:
        add("flagged", f"size: {len(files)} files > {MAX_FILES}", "*")
    if not files:
        add("flagged", "empty or unparseable patch", "*")

    if any(f["tier"] == "owner" for f in findings):
        verdict = "owner"
    elif findings:
        verdict = "flagged"
    else:
        verdict = "clean"
    return {"verdict": verdict, "changed_lines": changed, "files": sorted(files), "findings": findings}


if __name__ == "__main__":
    diff = open(sys.argv[1], encoding="utf-8", errors="replace").read() if len(sys.argv) > 1 else sys.stdin.read()
    result = check(diff)
    print(json.dumps(result, indent=2))
    sys.exit({"clean": 0, "flagged": 1, "owner": 2}[result["verdict"]])
