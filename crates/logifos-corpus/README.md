# Corpus grading

This unpublished workspace tool replays every entry in the local
`corpus/conformance/manifest.json`. It never fetches corpora or host fonts.
Run from the repository root:

```sh
cargo run --locked -p logifos-corpus
cargo run --locked -p logifos-corpus -- corpus/conformance/manifest.json --font math.ttf --sha256 <font-sha256>
cargo test --locked -p logifos-corpus
```

MathML runs through parse → transform → writer → parse/transform again
(exact Formula equality), then layout → SVG when a font is supplied.
Layout uses 18 px, script level 0, compact display, the supplied face 0
and no fallback fonts.
The provider validates the font and its supplied hash. Bootstrap requests
run unchanged through the current facade: their old versionless envelopes
now refuse `INVALID_REQUEST`, rather than exercising the MathML pipeline.
A refusal ends that case; subsequent stages are absent, never counted as passes.

Each case has one observed outcome: `pass`, `refusal` with stage/category,
or `failure`. No configured font is the explicit harness refusal
`layout/font_not_configured`; it does not claim a renderer refusal or layout
coverage. Stage counts keep successful syntax round trips measurable even
without a font. Engine refusals use stable categories, not diagnostic text;
layout uses its public wire error codes. Integrity errors, panics, invalid
writer output, Formula differences and unstable SVG emission are failures.
Expected manifest assertions remain separate from observed capability:
an expected refusal still counts as a refusal, never a coverage pass.

The deterministic JSON report contains sorted case ids, aggregate outcome
and category counts, stage counts, the manifest hash, per-case input hashes
and the optional font identity. It has no timestamps or local paths.
The regression snapshot covers the complete canonical manifest without a
font; a synthetic font exercises actual layout/emission in tests.
Exit 0 means no failures (typed refusals are allowed), exit 1 means case
failures, and exit 2 means a malformed manifest or configuration.
Missing files and hash mismatches fail individual cases and replay continues;
invalid versions, duplicate ids and empty manifests abort instead of
silently shrinking coverage. Fixture paths must stay inside the manifest's
directory and may not traverse symlinks. All JSON uses `logifos-json`.

This is a capability scoreboard over the locally available conformance inputs.
Pending expectations are not goldens, and the reference suites with unresolved
pins in `corpus/reference/sources.json` are not replayable inputs.
A passing pipeline does not establish upstream parity or visual correctness.
