# Logifos — design and architecture

Status: **in progress**. Every section states the current implementation
first, then the target. Nothing here is aspirational without a matching
task.

## 1. What logifos is

A Rust-native math formula renderer with three runtimes (Ruby, TypeScript
via WASM, Python). It renders the **Plurimath Formula model** — the central
semantic tree of the plurimath ecosystem — to deterministic SVG. Clean-room:
no Lasem relation and no Lasem code. JEuclid, Lasem, WebKit's MathML
renderer, MathJAX and KaTeX are study references; their test suites are
adopted as grading corpora (see §9).

## 2. The model layer — `logifos-model`

**Current.** `Formula` is a typed Rust tree:

```rust
enum Formula {
    Sequence(Vec<Formula>),                       // mrow / the root run
    Symbol(String),                               // mi, mn text
    Operator(String),                             // mo
    Fraction(Box<Formula>, Box<Formula>),         // mfrac
    Superscript(Box<Formula>, Box<Formula>),      // msup
    Subscript(Box<Formula>, Box<Formula>),        // msub
}
```

**The wire contract.** The boundary format is the plurimath common model:
`{"class": "Math::Formula", "fields": {…}}`, validated against a pinned
`FORMULA_CONTRACT` identity (`origin`, `revision`, `schema_path`, `sha256`
of the upstream definition). Conversion is all-or-nothing, budget-bounded
(`MAX_NODES = 100_000`, `MAX_DEPTH = 128`), and refuses typed with
`WireError { code: WireErrorCode, path }` — no partial trees.

**Target.** The remaining plurimath classes (fractions with attributes,
sub/sup pairs, tables, fences, function applications) land as typed
variants with the same refusal discipline. Adding a class is a wire-map
entry plus a layout arm — the two extensions every other stage is
insulated from.

## 3. The syntax layer — `logifos-syntax-mathml`

**Current.** A namespace-aware streaming parser (`parser.rs`: `math`,
`mi`, `mn`, `mo`, `mfrac`, `msub`, `msup`, `mrow`; limits on depth, nodes
and source bytes) builds a `MathmlDocument` syntax model (`model.rs`:
`Element`/`Node`/`Attribute`/`QualifiedName`, namespace declarations
tracked), and `transform.rs` lowers it conservatively into `Formula`.
Elements outside the current vocabulary refuse typed
(`TransformError::UnsupportedElement`), never silently drop.

**Target.** Full MathML Core coverage (msqrt, mtable/mtr/mtd, mfenced,
mspace, mtext, mstyle, merror) with the MathJax mml3-style implied-mrow
semantics, then LaTeX and AsciiMath syntax crates behind the same
transform contract.

## 4. The layout engine — `logifos-layout`

**Model.** Formula lowers to a **box tree**: `BoxKind::{Leaf, Rule, HBox,
VBox}` with metrics (`width`, `height`, `depth`), ink bounds, kerns and
stack items. Layout never mutates Formula, consults no global state, and
draws nothing: it produces geometry plus placed glyphs for a painter.

**Fonts.** `FontProvider` takes hash-pinned font resources (id + sha256 +
bytes; every resource verified before any glyph resolves) and a fallback
order. Metrics, the **OpenType MATH table** and **TrueType glyph outline
points** are read through the in-crate sfnt reader, which is
**Fontisan-backed** (`fontisan::sfnt`) — this is the multi-font seam:
different fonts are different pinned resources, never a bundled constant.
Outline points decode for every glyph at load (simple glyphs and depth-capped
composites), so a malformed glyph is an `InvalidFont` before any layout runs,
and a glyf face that claims ink bounds without contours refuses to load —
the painter's invariant "no contours, no ink" is enforced upstream. CFF/CFF2
faces carry no decoded points yet and are a typed gap (`OutlinesUnavailable`),
not a silent blank.

**Style.** `LayoutOptions { font_size_px, script_level, display }` resolves
through the MathML Core §4.5 script-level schedule and the OpenType MATH
percentages into `ResolvedStyle`, which scripted children raise one level
with compact displaystyle (§3.4.4) and a cramped math-shift flag.

**Current.** Sequences, tokens (mi/mn/mo glyph placement with italics
correction), operator spacing (a validated operator set with MathML
spacing classes B/C/D), fractions and scripts lay out. Fractions follow
MathML Core §4.2: axis-aligned rule at the MATH `FractionRuleThickness`,
children centered, shifts grown to keep the minimum gaps, with the
display variants selected by `LayoutOptions::display`. Scripts follow
§3.4.1: the script child builds one level up and compact, the shift is
the MATH-constant max over the ink clamps, the base's italic correction
kerns before the script, `SpaceAfterScript` trails it, and the script
re-baselines through a padded vbox wrapper. Radicals follow §3.3.3:
the surd stretches vertically to rule + gap + radicand ink height
(variant or extender-grown assembly with min-connector overlap), the
overbar rides the radicand at a zero advance, and the mroot index
raises by the bottom-raise percent two levels up. Both set math-shift
compact; the index is also compact displaystyle.

**Target.** Tables (row/column alignment), stretchy delimiters
(horizontal glyph assembly with min connector overlap — the
`expand_stretch` machinery already stretches vertically).

## 5. The painter — `logifos-svg`

**Current.** `emit(&BoxTree, &[PlacedGlyph], &SvgOptions { color, title })`
walks the tree (depth-first, stable ids), paints rules as `<rect>`, then
paints the glyph layer: one `<path>` per placed glyph, TrueType quadratics
with implied on-curve midpoints (including across each contour's wrap),
flipped into top-left coordinates around the glyph's baseline origin.
Contourless glyphs paint nothing. The painter first converts the backend's
pixel-scaled TrueType points into `outline::GlyphOutline`: ordered, closed
`PathCommand::{MoveTo, LineTo, QuadTo, CubicTo, Close}` contours. The typed
outline also accepts synthetic cubic segments without depending on a font
encoding. `to_svg_path` places endpoints and controls at the baseline origin
and flips y once; invalid contour order, non-finite input and placement
overflow return typed errors with no partial output.

The output is byte-stable: shortest round-trip binary64 numbers without
quantization, normalized negative zero, lowercase color, no timestamps,
boxes first and a trailing `glyphs` group last. Path data uses absolute
`M`/`L`/`Q`/`C`/`Z` commands, single spaces between numbers and after drawing
commands, and no space after `Z`. Contour order and winding are preserved.

**Target.** CFF cubic paths when Fontisan adds charstring decoding,
glyph paths positioned inside their owning box groups (paint-order
fidelity for overlapping ink), ink-extent viewports, optional title.

## 6. The facade — `logifos-facade`

One JSON boundary for every runtime, native and WASM:

* Envelope: `abi_version`/`schema_version` negotiation; closed-field
  validation per op; unknown options refuse (`UNKNOWN_OPTION`); caller
  data is never echoed in refusals.
* Ops: `negotiate` (capabilities + limits), `parse` (mathml source → wire
  model + contract echo), `render` (wire Formula + pinned contract + font
  resources → SVG bytes), `transform` (syntax↔Formula, pending the syntax
  contract).
* Limits: request/response bytes, JSON depth, font count/bytes, source
  bytes — lowered per request, never exceeded silently.
* Fonts: base64 resources with sha256 verification, canonical base64
  checks, fallback-order validation.
* Transports: an owned-buffers C ABI (`include/`) and the WASM entry,
  byte-identical to `request_json`.

## 7. Reference-renderer study

| Renderer | Lesson adopted |
| --- | --- |
| MathJax | The MathML→internal transform semantics (implied rows, mstyle inheritance, the mml3 element coverage) and the node-factory pattern. |
| WebKit MathML | Per-construct layout classes and the OpenType MATH table discipline (axis height, radical parameters, script shifts). |
| KaTeX | Box-model strictness (vbox/hbox/kern/rule as the only primitives) and the TeXbook spacing algorithm's explicitness. |
| JEuclid | The attribute-inheritance cascade (mathcolor/mathsize/mathvariant) and its MathML conformance traps. |
| Lasem | The consumer-facing feature envelope only — retired, no code relation. |

## 8. Testing architecture

* Per-crate unit tests (today: 77 green across the workspace).
* The plurimath-testsuite mathml stage + the vendored W3C MathML suites as
  the parse/round-trip corpora.
* **Reference-implementation corpora (in adoption):** KaTeX's test corpus,
  the MathJax test suite, JEuclid's tests, the WebKit MathML layout tests
  and the lasem-ruby fixtures, vendored with pinned provenance into
  `corpus/reference/` and replayed through parse→layout→emit, with a
  per-case pass/fail scoreboard as the SOTA gate.
* Golden-SVG structural diffs against MathJax/JEuclid renders of the same
  inputs; raster comparison via claricle/svgpng as the second gate.

## 9. Operations

Release trains with lockstep versions across the workspace and the three
runtime repos, fuzz targets on every parser and the font reader, benches
with a perf-gate, `MIGRATION.md` for the wire contract evolution — the
parsanol/leptris family discipline verbatim.
