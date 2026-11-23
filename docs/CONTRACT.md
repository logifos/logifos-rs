# Facade wire contract, version 1

This is the target portable contract. The current Rust facade is a bootstrap
stub and does **not** implement it. The checker validates authored transport
messages; its results are not parser, renderer, font-backend or ABI execution
results. No successful engine outputs or geometry baselines are supplied here.

## Authority and external payloads

`contracts/request.schema.json` and `response.schema.json` define closed JSON
envelopes. `contracts/check.py` adds byte, encoding, identity and resource checks.
`contracts/pins.json` keeps Formula and MathML syntax contracts separate from
these envelopes. A contract identity has `origin`, full 40-character Git
`revision`, repository-relative `schema_path`, and lowercase SHA-256 `sha256`
of the exact original schema file. Record the immutable upstream revision,
never a branch or a mutable URL. Resolve references relative to that pinned
schema; record their pins too before validation is enabled.

The Formula origin is the upstream Plurimath Rust project. Its accepted schema
path/revision/hash remain null pending card 12's authoritative boundary decision;
the MathML syntax pin remains null pending card 13. Null pins prohibit payload
qualification with `UNPINNED_CONTRACT`. Supplying a different descriptor is
`CONTRACT_MISMATCH`. This card defines no Formula classes, fields, conversion
mapping or shared-model extension. The checker refuses external payloads even
when pins are filled until the owning card supplies the actual schema validator.
Meaningful inherited presentation and child order must survive transformation;
unsupported content is refused, never dropped or reconstructed from old source.

## Bytes, versions and operations

Every request and response has integer `abi_version: 1` and `schema_version: 1`.
The ABI version governs transport ownership; the schema version governs JSON
meaning. Versions match exactly, with no downgrade or best-effort decoding.
`negotiate` has no operation-specific fields and returns `kind: capabilities`,
version lists, implemented operation/output lists, external contract identities
(or null), and effective hard limits. An advertised version does not imply an
implemented operation. Until its behavior is implemented, an operation returns
`NOT_IMPLEMENTED` and is absent from the implemented list.

Messages are length-delimited UTF-8 JSON, without a BOM or NUL terminator.
Duplicate keys, malformed UTF-8, unpaired Unicode surrogates, nonfinite numbers,
and trailing non-whitespace bytes fail. Strings preserve Unicode scalar values;
no NFC normalization is implied. JSON object-key order has no meaning. Byte
limits and error offsets count UTF-8 bytes, never UTF-16 units or characters.
The error `path` is a JSON Pointer (RFC 6901); the empty string means the whole
message. Unknown envelope fields are `INVALID_REQUEST`; unknown option keys are
`UNKNOWN_OPTION`. All objects are closed unless an external schema owns them.

| Operation | Required operation fields | Qualified result |
| --- | --- | --- |
| `parse` | `format: mathml`, `source: string` | `kind: syntax`, format, `syntax_contract`, opaque `value` |
| `transform` | `format: mathml`, `direction: syntax_to_formula` or `formula_to_syntax`, `input`, `input_contract`, `output_contract` | `kind: formula` or `syntax` and matching external identity |
| `render` | `formula`, `formula_contract`, `output: svg` or `png` | `kind: svg` only |

Syntax-to-Formula and Formula-to-syntax conversions use the corresponding pinned
input/output schema in that order. Parse returns syntax, not a separately
invented Formula mapping. A later facade convenience API may compose these
operations without bypassing either boundary. LaTeX and AsciiMath are not v1
formats. A request using a supported envelope with unsupported syntax receives
`UNSUPPORTED_SYNTAX` from its owning implementation.

`output: png` always returns `UNSUPPORTED_OUTPUT` from the core after envelope
validation, before font loading or model/layout work. PNG belongs to runtime
rasterization of a qualified SVG through its declared converter. The core does
not return empty PNG bytes, an SVG mislabeled as PNG, or a success placeholder.

## Per-call options and limits

Optional `options` are local to one call. Omitting them means:
`font_size_px: 16`, `display: inline`, `color: #000000`, `fonts: []`,
`fallback_order: []`, and the hard ceilings below. A call cannot mutate defaults
or resources of another call. Limits are an initial contract budget, not measured
runtime guarantees. Implementations must enforce them and may advertise lower
ceilings during negotiation. Callers may only lower limits; exceeding an
advertised bound is `RESOURCE_LIMIT`. An invalid or raised requested ceiling is
`INVALID_REQUEST`. `font_size_px` is finite and in `(0, 4096]`; display is
`inline|block`; color is a six-digit RGB hex string.

| Bound | Hard ceiling |
| --- | ---: |
| Entire request bytes | 8,388,608 |
| JSON container depth (root object is 1) | 64 |
| UTF-8 source bytes | 1,048,576 |
| External tree nodes / depth | 100,000 / 128 |
| Font resources | 16 |
| Decoded bytes per font / total | 4,194,304 / 4,194,304 |
| Entire response bytes | 8,388,608 |
| Positioned glyphs | 100,000 |

The request byte ceiling includes base64 expansion. JSON depth is checked before
recursive decoding. The tree-node/depth and glyph bounds are enforced in the
owning parser/model/layout stage; this envelope checker cannot qualify those
stages. XML DTDs, entities, network resources and filesystem resolution must be
refused by the syntax parser. No request accepts a resource path or remote URL.
Response limits apply to the complete encoded envelope, not just SVG text;
return a bounded `RESOURCE_LIMIT` response instead of a partial success. No
wall-clock guarantee or process-global cache mutation is part of this version.

## Caller fonts and fallback

A font resource is `{id, sha256, bytes_base64, face_index}`. Bytes use canonical
padded RFC 4648 base64, with no whitespace or URL alphabet. SHA-256 identifies
the full decoded supplied file; `face_index` selects a face within that file.
IDs are per-call ASCII identifiers and must be unique. The full resource key is
`(sha256, face_index)`, never a filename or family-name string. No fonts or metrics
are bundled, fetched or found on the host. Rendering requires nonempty caller
font resources.

`fallback_order` must list every resource ID exactly once; array order is the
lookup order, with the first font supplying primary math constants. Fontisan
must validate every face, outlines and required MATH data before layout. Invalid
bytes/face/MATH data produce `INVALID_FONT`; hash mismatch produces
`FONT_HASH_MISMATCH`. The wire checker verifies byte identity and list integrity
only and cannot assert font-file validity. Font-backend qualification is card 17.
Select the first font with the required glyph/variant in caller order; never use
host fallback. If no supplied font covers a required glyph, return
`MISSING_GLYPH`, with no partial SVG. Fallback does not override primary MATH
constants. Responses list actually used resource identities in first-use order.
Fonts supplied for parse/transform have no semantic effect and still undergo
identity/resource validation. No resource survives the call unless a separately
specified immutable cache keys every relevant identity and option.

## SVG and metrics

A successful render result contains `kind: svg`, `media_type: image/svg+xml`,
`encoding: utf-8`, the Unicode `svg` string, `metrics` and `fonts_used` identities.
SVG must be self-contained, with no scripts, event handlers, external references,
network font fetches or host-font text. Outline generation and SVG safety require
structural checks in the engine's SVG card; this checker refuses successful SVG
qualification because it does not implement those checks.

All returned metrics and SVG root user units are CSS pixels (`unit: css_px`).
One root user unit equals one CSS px; the root viewport/viewBox must preserve that
mapping. Font design units are scaled by `font_size_px / unitsPerEm` before
layout. The layout origin is the top-left of the advance box; x increases right
and y increases down. `width` is horizontal advance, `height` is the nonnegative
baseline distance from the origin, `depth` is nonnegative distance below it,
and `baseline == height`. The logical vertical extent is `height + depth`.
`ink_bounds` has finite x/y and nonnegative width/height in that same coordinate
system; ink may overhang the logical box. Ink must never be clipped merely to
hide an overhang. Any viewport padding/translation is represented explicitly in
the SVG and reconciled to these unchanged logical metrics. All metric numbers are finite and within binary64 range;
normalize negative zero to zero in serialized results. Raster DPI is outside the
core contract. Deterministic output requires equal pins, bytes, resource order,
options and engine artifact, not merely equal source strings.

## Structured failures and ABI ownership

A failure is `{schema_version, abi_version, ok:false, error:{code, message,
path, stage}}`. A success has `ok:true` and `result`, never both result and error.
Codes and stage vocabulary are enumerated by the response schema. Messages are
bounded diagnostics, not stable assertion strings and must not echo font bytes,
full caller source or credentials. Failure precedence is byte/depth/UTF-8/JSON,
versions, closed envelope/options, requested limits, core PNG refusal, resource
identity, external pins/schema, then operation execution. The checker returns
exit 0 for a checked wire envelope, 1 for a contract refusal, 2 for I/O failure.
None of these is an engine execution verdict.

The native transport planned for card 24 takes `(const uint8_t *ptr, size_t len)`
and returns a transport status plus an owned response `(uint8_t *ptr, size_t len)`.
A null input with positive length is a transport `INVALID_REQUEST`; null/zero
is an empty message and fails JSON decoding. The caller retains input ownership;
the callee neither writes nor retains those bytes. The caller must release a
successful response allocation exactly once through the matching exported free
from the same library; pointers, length and allocator must not be mixed across
artifacts. A null response on allocation failure has length zero and a transport
`ALLOCATION_FAILED` status; no JSON allocation is promised in that case. Panics
must not unwind across C, WASM or runtime boundaries. Invalid foreign output
pointers are caller ABI misuse, not recoverable JSON errors. The exact C symbol
names/header, panic containment and WASM ownership implementation belong to their
transport cards and are not established by this document.

## Focused checks and sources

Run `python3 contracts/test_contract.py` for named fresh envelope/refusal and
metric-unit checks, or `python3 contracts/check.py contracts/cases/parse.json`.
No test invokes an engine, imports a renderer fixture or supplies a successful
render baseline. The existing bootstrap API is deliberately not rewritten here.
`contracts/provenance.json` records authorship for this card's paths. The shared
card-05 provenance checker is not present at this branch's base and must be run
by integration once available.

Direct specification sources read for this contract:

- [RFC 8259 (December 2017), sections 4, 6, 8](https://www.rfc-editor.org/rfc/rfc8259): JSON, numbers, Unicode.
- [RFC 4648 (October 2006), sections 3–4](https://www.rfc-editor.org/rfc/rfc4648): base64 byte encoding.
- [SVG 2, chapter 8](https://www.w3.org/TR/SVG2/coords.html): coordinate systems and user units; the CSS-pixel choice above is Logifos policy.
- [OpenType 1.9.1, OpenType font file](https://learn.microsoft.com/en-us/typography/opentype/spec/otff): file/face and design-unit context; no font bytes or metrics were imported.
