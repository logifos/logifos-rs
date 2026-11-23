# Formula wire boundary, revision 1

The existing six-variant `Formula` tree accepts a strict subset of the
[Plurimath Rust facade](https://github.com/plurimath/plurimath-rs/tree/74dbffe4c50e5ed33d25e0f29caddf7e8db776ad/plurimath-facade)
common-node contract at revision `74dbffe4c50e5ed33d25e0f29caddf7e8db776ad`.
`FORMULA_CONTRACT` exposes the immutable repository, revision, definition path
and SHA-256 of `plurimath-facade/src/model.rs`:
`6fc084ad002863b5910b556b8a18b3ebf666b147b8a4d700202460fa3402e7e1`.

Field and class definitions were checked directly in that revision's
`src/model.rs`, `src/parser.rs`, `src/xml_parse.rs`, `src/symbols.rs` and
`src/writer.rs`, relative to `plurimath-facade`. The upstream authority is Rust
source, not a standalone JSON schema. This pin must not be advertised as an
upstream schema path/hash or as qualification of a portable facade envelope.

Every node is a closed object containing exactly `class` and `fields`.
Every listed field is required. No omitted value is defaulted.

| Formula variant | Exact wire class | Exact fields |
| --- | --- | --- |
| `Sequence` | `Math::Formula` | `value`: ordered node array |
| `Symbol` | `Math::Symbols::Symbol` | `value`: string, preserved exactly |
| `Fraction` | `Math::Function::Frac` | `parameter_one`, `parameter_two`: nodes |
| `Superscript` | `Math::Function::Power` | `parameter_one`, `parameter_two`: nodes |
| `Subscript` | `Math::Function::Base` | `parameter_one`, `parameter_two`: nodes |
| `Operator` | `Math::Symbols::Plus`, `Minus`, `Equal`, `Asterisk`, `Times` (each with the same namespace) | `value`: explicit null |

Operators use typed strings `+`, `-`, `=`, `*`, `×`, respectively. A generic
symbol containing one of those strings remains a symbol; class identity is
never inferred from its contents. Other operator strings refuse on output.
The two binary fields preserve operand order: numerator/denominator,
base/exponent, and base/index, respectively.

Only the fields listed above fit the current model. Every other field,
including an explicit null, empty container or upstream optional metadata,
returns a typed refusal at its JSON Pointer. Unsupported classes, malformed
children and exceeded budgets also refuse the entire conversion. No partial
result, opaque fallback, coercion, normalization or reconstruction is returned.
The default upstream Formula constructor includes extra fields and therefore
requires a future model revision before its output can be accepted unchanged.
This is subset support, not full upstream or MathML preservation.

## Migration

Replace the bootstrap sequence field `parameter1` with `value` and the bootstrap
class `Math::Symbol` with `Math::Symbols::Symbol` in newly constructed payloads.
Do not strip extra fields from existing payloads to force acceptance.
`Formula::from_wire` now returns `WireError` rather than `String`; callers can
match `code` and `path`, or use `Display` for diagnostics. `TryFrom<&Value>`
handles nonobject roots; `to_wire` returns `Result<Value, WireError>`.

Raw JSON decoding belongs to `logifos-json`; this crate converts already decoded
values. Successful conversions round-trip every accepted field and class
exactly (JSON object order is immaterial). Both directions enforce a root-inclusive
depth of 128 and a total of 100,000 nodes per conversion. Future shared-model
extensions require an explicit versioned contract update; no such fields are
introduced here.
