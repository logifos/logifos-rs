# Changelog

## Unreleased

MathML transforms and writes attribute-free style, error, padded and phantom
containers while preserving their kinds. Grouping containers use row layout;
phantoms retain dimensions without painting glyphs or rules. Semantics uses its
first presentation child and ignores annotation payloads. Presentation
attributes and standalone annotations still refuse conversion.

The SVG painter exposes validated glyph-local move, line, quadratic, cubic and
close commands, consuming the existing Fontisan-backed TrueType contours.
Path emission preserves deterministic number formatting and contour order and
refuses non-finite coordinates or placement overflow without partial output.

The repository contains an unpublished Rust workspace with six crates for JSON
requests, formula models, MathML syntax, layout, SVG output and a request facade.
JSON decoding checks input size and nesting. The model converts sequences and
symbols from the Plurimath wire representation. Parsing requests currently return
an error; layout and SVG types are placeholders.

GitHub Actions defines formatting, build, test, lint, documentation, dependency
and spelling checks. Local formatting and spelling checks can run through the
repository hook or pre-commit. Contribution and security guidance is available
in the repository root.
