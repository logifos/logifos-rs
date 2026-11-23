# CLAUDE.md

## What logifos-rs is

A Rust workspace rendering math formulas to SVG. The central model is the
Plurimath Formula model (the `{class, fields}` source contract of
plurimath-rs), mirrored as a typed Rust tree in `logifos-model`. Syntax
crates transform syntax models into Formula and back; `logifos-layout`
lowers Formula into a box tree; `logifos-svg` emits deterministic SVG.
Fonts and the OpenType MATH table come through Fontisan; PNG is delegated
to claricle/svgpng. Clean-room — no Lasem code.

## Commands

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Conventions

* Every runtime enters through `logifos-facade`'s JSON boundary; ops are
  explicit and every request carries its options. The boundary never
  panics — it refuses typed.
* All JSON decoding goes through `logifos-json` (depth and size bounds in
  one place).
* Adding a syntax is a new `logifos-syntax-*` crate plus its transformer;
  the layout and rendering core never changes (OCP).
* SVG output must be byte-stable: fixed ordering, no timestamps.
* License MIT; authors Ribose Inc.
