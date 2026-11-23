# logifos-rs

Rust-native math formula renderer. Syntaxes parse into syntax models,
transform into the central Formula model, get laid out into a box tree,
and render to deterministic SVG. Fonts come through Fontisan; PNG output
rides claricle/svgpng. Runtimes: Ruby, TypeScript (WASM), Python.

Clean-room: no Lasem relation and no Lasem code. JEuclid, Lasem, WebKit
MathML, MathJAX, and KaTeX are architecture study references only.

See [docs/PLAN.adoc](docs/PLAN.adoc) for the architecture, crates and
phased plan.

Development setup and optional local hooks are described in
[CONTRIBUTING.md](CONTRIBUTING.md). See [CHANGELOG.md](CHANGELOG.md) for the
unreleased workspace status and [SECURITY.md](SECURITY.md) for the reporting gap.
