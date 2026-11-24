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

* `../canon/` compares logifos's renders against reference-renderer
  artifacts for the shared cases: `scripts/fetch-canon-mathjax.sh`
  installs the pinned MathJax, `scripts/render-canon-mathjax.mjs`
  renders every manifest case into `canon/mathjax/`, and
  `cargo run -p logifos-corpus -- --canon mathjax` grades the corpora,
  captures logifos's own SVGs, and prints the structural diff with
  pass-rate counts; redirect to `canon/report.json` for the committed
  snapshot. The same flow exists for JEuclid
  (`scripts/fetch-canon-jeuclid.sh` builds the pinned rototor fork with
  a raised source level and opened java.base modules,
  `scripts/render-canon-jeuclid.mjs` renders all cases in one JVM,
  `--canon jeuclid` compares; `canon/report-jeuclid.json`).

* `../fonts.json` pins the grading font (STIX Two Math from google/fonts,
  fetched to `corpus/fonts/` by the same script). With it materialized,
  `--reference` replays through layout and SVG emission with real glyph
  outlines; the first run rendered 85 lasem cases end to end.
* The plurimath-testsuite mathml stage and the vendored W3C MathML suites
  (in plurimath-testsuite) remain the primary parse/round-trip corpora.

The offline [grading tool](../crates/logifos-corpus/README.md) replays every
case in the canonical conformance manifest and reports observed outcomes
and stage counts as deterministic JSON. Run `cargo run --locked -p logifos-corpus`.
