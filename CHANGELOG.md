# Changelog

## Unreleased

The repository contains an unpublished Rust workspace with six crates for JSON
requests, formula models, MathML syntax, layout, SVG output and a request facade.
JSON decoding checks input size and nesting. The model converts sequences and
symbols from the Plurimath wire representation. Parsing requests currently return
an error; layout and SVG types are placeholders.

GitHub Actions defines formatting, build, test, lint, documentation, dependency
and spelling checks. Local formatting and spelling checks can run through the
repository hook or pre-commit. Contribution and security guidance is available
in the repository root.
