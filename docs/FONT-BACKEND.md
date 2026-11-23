# Font metrics and MATH

`logifos_layout::font` loads caller-supplied OpenType sfnt fonts and collections
through Fontisan, pinned in Cargo.toml and Cargo.lock. It exposes design-unit
metrics and checked MATH data without filesystem access or host fallback.

A `FontProvider` belongs to one call. Its input contains decoded bytes, a
lowercase SHA-256 string, an ASCII resource ID, and a face index. The caller's
fallback order must name every resource exactly once. The provider checks all
resources before resolving a character, including resources that remain unused.
The byte ceiling is 4 MiB in total, with at most 16 resources. No font data or
cache survives through global state.

Character lookup uses Fontisan's Unicode cmap selection: Windows full Unicode,
then Windows BMP, then the first Unicode-platform record. Selected subtables
must use format 4 or 12. Metrics come from head, hhea, maxp and hmtx. In hmtx,
the final long metric supplies advances for trailing side-bearing entries.
Collections use the full supplied file hash and selected face index as identity.

`resolve` selects the first covering font and converts its advance, side bearing,
italics correction, and top-accent attachment to CSS pixels. Missing top-accent
entries use half the advance; missing italics entries use zero. Scaling uses
`font_size_px / units_per_em`. Percentage constants remain percentages. Primary
math constants always come from the first font in fallback order. Accessing the
primary font or resolving a glyph records its identity in first-use order;
alias IDs for the same file and face do not duplicate that identity.

MATH readers support all 51 MathValueRecord constants, the five other constants,
coverage formats 1 and 2, italics corrections, top-accent attachments, vertical
and horizontal variants, and assembly parts. Parts retain their specified order
and extender flags. Extended-shape coverage and math kern structures are
validated but are not exposed for layout yet. Device tables are bounds-checked;
device and variation adjustments are ignored at the default variation instance
for resolution-independent metrics. A decoded-record budget also bounds work
when many offsets share the same subtable.

`resolve_stretch` searches fonts in caller order for a sufficiently large variant
or an assembly description. It returns the owning font identity, connector
overlap, and design-unit data. Expanding extenders and placing assembly parts
remain layout responsibilities.

## Remaining work

Fontisan exposes raw outline byte ranges. The backend checks those ranges and
TrueType loca bounds, but does not decode drawable glyf or CFF paths. `outline`
returns typed `OutlinesUnavailable`; metrics loading does not qualify a font for
rendering. Full outline validation, assembly placement, and facade rendering
integration remain required before successful SVG output. WOFF/WOFF2 input and
non-default variation instances are not supported by this adapter.

Tests construct small fonts and collections in Rust, with separately chosen
specification values. No font binaries or renderer outputs are stored.

## Specifications

- [OpenType file structure](https://learn.microsoft.com/en-us/typography/opentype/spec/otff)
- [Horizontal metrics](https://learn.microsoft.com/en-us/typography/opentype/spec/hmtx)
- [Character mapping](https://learn.microsoft.com/en-us/typography/opentype/spec/cmap)
- [MATH](https://learn.microsoft.com/en-us/typography/opentype/spec/math)
- [Coverage and device formats](https://learn.microsoft.com/en-us/typography/opentype/spec/chapter2)

## Glyph measurement

`glyph_bounds` exposes checked default-instance TrueType control-point bounds;
`measure` combines them with selected-font hmtx advances and side bearings into
CSS pixel metrics and an enclosing ink rectangle. Empty and zero-contour glyphs
retain advance without ink or vertical extent. CFF/CFF2 bounds refuse with
`MetricsUnavailable`; missing glyph coverage, including cmap glyph zero, refuses
with `MissingGlyph`. These measurements do not implement drawable outlines.
See [token and row layout](TOKEN-ROW-LAYOUT.md) for coordinates, sizing, spacing,
limits and typed refusal behavior.
