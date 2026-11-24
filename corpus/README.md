# Logifos corpora

* `reference/` — the reference-renderer test suites (KaTeX, MathJax,
  JEuclid, WebKit MathML, lasem), pinned to audited revisions in
  `sources.json` and fetched by `scripts/fetch-reference-corpus.sh`
  into gitignored per-corpus directories. `cargo run -p logifos-corpus
  -- --reference` flattens every suite into manifest cases, replays
  them through the same parse → transform → write → roundtrip → layout
  → emit pipeline as the conformance manifest, and prints the
  per-case scoreboard; redirect to `reference/report.json` to refresh
  the committed snapshot. Cases that complete the full roundtrip but
  stop at the font gate show as `layout/font_not_configured`
  refusals — their capability depth is visible in each source's
  `stage_counts`.
* The plurimath-testsuite mathml stage and the vendored W3C MathML suites
  (in plurimath-testsuite) remain the primary parse/round-trip corpora.

The offline [grading tool](../crates/logifos-corpus/README.md) replays every
case in the canonical conformance manifest and reports observed outcomes
and stage counts as deterministic JSON. Run `cargo run --locked -p logifos-corpus`.
