# Logifos corpora

* `reference/` — the reference-renderer test suites (KaTeX, MathJax,
  JEuclid, WebKit MathML, lasem-ruby), pinned in `sources.json` and
  fetched by `scripts/fetch-reference-corpus.sh`. Each suite replays
  through parse → layout → emit with a per-case pass/fail scoreboard:
  the SOTA gate.
* The plurimath-testsuite mathml stage and the vendored W3C MathML suites
  (in plurimath-testsuite) remain the primary parse/round-trip corpora.
