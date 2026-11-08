# Contributing to Vorrat

We don't take pull requests. [Open an issue](../../issues/new/choose) that
describes the bug or the idea, and our agents usually turn it into a fix
quickly. A maintainer reviews and merges every change.

Diagnoses and proposed fixes are welcome in the issue: the steps that show the
problem, the code you suspect, or a patch as a diff snippet. If your analysis
shapes the fix, we credit you in the pull request that lands it.

Translations are welcome the same way: paste corrected lines from
`locales/<language>.tsv` into an issue.

Patches follow the rules in [`AGENTS.md`](AGENTS.md). One that people often
miss: comments only as a doc comment at the top of a function, struct, enum or
module, never inside code, attributes, struct definitions, CSS rules or
templates, and never with pull request or issue numbers.
`cargo test --test style` checks it.

To build Vorrat yourself, see [`README.md`](README.md) and
[`AGENTS.md`](AGENTS.md).
