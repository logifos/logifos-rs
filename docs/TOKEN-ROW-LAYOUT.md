# Token and row layout

`logifos_layout::layout::layout` accepts a typed `Formula`, a caller-created
`FontProvider`, and `LayoutOptions { font_size_px, script_level }`. It returns
validated box geometry, glyph placements in input order, and the effective
style. Glyph pen positions use the root box's top-left coordinate system, with
y increasing down; each pen's y is its alphabetic baseline. Each placement
retains the font resource ID, file/face identity, glyph ID, and effective size.
The box tree remains purely geometric. No drawing or facade route is added.

Supported Formula variants are `Sequence`, `Symbol` and `Operator`. Symbols
retain their exact scalar order and use cmap-selected glyphs and hmtx advances.
There is no normalization, implicit italic alphabet substitution, GSUB/GPOS
shaping, bidi processing, or line breaking. This is a horizontal mathematical
run API, not a general text shaper. A Symbol containing `+` stays a Symbol.
The five operator strings accepted by the model's wire contract are supported:
`+`, `-`, `=`, `*`, `×`. Other directly constructed operators refuse. In
particular, the stored `-` selects that exact character, without substitution.
Fractions, superscripts and subscripts return `UnsupportedConstruct` with their
specific variant, including when nested in a row.

## Explicit size and script level

The current model has no presentation fields. Options apply uniformly to every
supported descendant; there are no invented Formula fields or per-node style
overrides. Direction, color and variant inheritance await a shared-model
contract. Sizes are finite CSS pixels in (0, 4096], measured at level zero.
Script level is an absolute nonnegative `u8`, also for a standalone token.

The effective size follows [MathML Core 4.5](https://www.w3.org/TR/2025/CR-mathml-core-20250624/#x4-5-the-math-depth-property),
starting at depth zero. Level one uses the primary font's script percentage;
level two uses its script-script percentage directly relative to level zero.
Every further level multiplies by 0.71, in ascending level order. The percentage
fields come from [OpenType MATH](https://learn.microsoft.com/en-us/typography/opentype/spec/math#mathconstants-table),
not from glyph design-unit scaling. This API requires positive percentages
no greater than 100, with script-script no greater than script. Invalid values
return `InvalidMathConstants`; effective size underflow returns `InvalidOptions`.
Fallback glyphs retain the primary font's effective size, scaled with the
selected font's own units per em.

## Operator spacing

The following narrow rules implement the form/category lookup in
[MathML Core 3.2.4.2](https://www.w3.org/TR/2025/CR-mathml-core-20250624/#dictionary-based-attributes)
and [appendix B.1](https://www.w3.org/TR/2025/CR-mathml-core-20250624/#operator-dictionary).
They are expressed as rules for the supported vocabulary, not an imported
operator dictionary. An operator first among multiple nonempty row children
has prefix form; a last operator has postfix form; otherwise it is infix.
Empty subtrees are inert for this API's form calculation. A single-operator
row, including nested single-operator wrappers and empty siblings, passes its
operator identity to its parent. Its margins are applied only at the outer
non-operator row. A standalone operator is placed in an implicit root row.

| Operator | Prefix | Infix | Postfix lookup fallback |
| --- | --- | --- | --- |
| `+`, `-` | 0 | 4/18 em each side | Infix |
| `*`, `×` | Infix lookup fallback | 3/18 em each side | Infix |
| `=` | Infix lookup fallback | 5/18 em each side | Infix |

Margins scale with the effective size. Increasing script level does not itself
suppress these dictionary margins. At a slanted-to-nonslanted transition, the
previous token's italic correction advances the cursor. Consecutive slanted
tokens do not add a correction between them. A token uses its final glyph's
correction; an operator is nonslanted. Rows pass the final correction outward,
so the root includes it exactly once in its final advance. This follows the
cursor treatment in [Core 3.3.1.2](https://www.w3.org/TR/2025/CR-mathml-core-20250624/#layout-of-mrow).
Italics corrections are signed; invalid resulting geometry refuses.

## Measurement and refusals

Advances and left side bearings come from
[OpenType hmtx](https://learn.microsoft.com/en-us/typography/opentype/spec/hmtx).
TrueType glyph bounds come from checked
[glyf headers](https://learn.microsoft.com/en-us/typography/opentype/spec/glyf#glyph-headers)
at the default variation instance, through Fontisan's glyph byte ranges. These
control-point rectangles enclose the outlines; they need not be tight curve
bounds. Their horizontal placement uses the hmtx side bearing, even if xMin
differs. Height is max(yMax, 0); depth is max(-yMin, 0). Ink rectangles preserve
overhangs and gaps above or below the baseline. Empty/zero-contour glyphs have
no ink and zero vertical extent while retaining their horizontal advance.

This bounds reader does not validate or decode outline instructions or component
paths, and does not qualify a font for drawing. CFF/CFF2 bounds currently return
`MetricsUnavailable` / `MISSING_METRICS`; font-wide ascent/descent are never used
to invent glyph bounds. Missing or malformed required tables refuse during font
loading. A cmap result of glyph zero counts as missing coverage. The first
covering resource is selected in caller order; missing bounds in that resource
refuse rather than silently skipping it. No host lookup or replacement glyph
is used. Missing italics entries use zero, as documented by the font backend.

Layout errors preserve font/geometry errors and distinguish unsupported
constructs/operators, invalid options/constants and resource limits. Both
model limits (128 root-inclusive levels and 100,000 Formula nodes) apply to
directly built trees as well as decoded inputs. A separate 100,000-glyph budget
bounds token measurement. There is no returned partial layout. Provider usage
tracking can reflect work performed before a refusal; use a fresh per-call
provider as required by the font API.

Arithmetic follows input order with checked finite geometry and canonical
signed zero. Resources, glyphs and children retain deterministic order. Tests
build fresh synthetic font bytes in memory, compare fixed box snapshots and
independently calculated advances, margins, ink extents and baselines. No font
binaries are included.
