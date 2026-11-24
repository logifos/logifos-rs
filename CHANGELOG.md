# Changelog

## Unreleased

MathML syntax also retains the standard font-variant names, red/blue colors,
positive named operator rspace values, boolean over-script accent flags and
explicit centered table-cell alignment. Writing preserves their owning elements;
other values and misplaced attributes refuse. Layout supports explicit centered
cells and refuses font variants, colors, explicit operator spacing and accents
with NOT_IMPLEMENTED until their rendering semantics are implemented.

MathML preserves root display mode and row-like displaystyle and scriptlevel
hints as typed presentation data. Validation accepts case-insensitive display
and boolean values and absolute or signed decimal script levels up to 255;
unsupported values and attributes still refuse at their exact locations.
MathML writing round-trips these hints, and layout applies inherited style,
relative depth scaling and absolute resets, including negative effective depths.
Fractions now adjust child size and compact style automatically. Operator
spacing and fence sizing retain the operator's effective font size; SVG uses
the resulting glyph geometry. Table rows and cells reject attributes instead
of discarding them. Presentation attributes outside the supported subset remain
unsupported.

MathML transforms and writes attribute-free style, error, padded and phantom
containers while preserving their kinds. Grouping containers use row layout;
phantoms retain dimensions without painting glyphs or rules. Semantics uses its
first presentation child and ignores annotation payloads. Presentation
attributes outside the supported display/script subset and standalone
annotations still refuse conversion.

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
