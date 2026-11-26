use logifos_model::Formula;
use std::{collections::BTreeMap, path::Path};

/// Tallies, over the derived plurimath-testsuite cases, which stage refuses
/// each case on the wire path — the follow-up wire slices' work list.
/// Ignored: a reporting tool, not a gate. Run with the corpus fetched:
///   cargo test -p logifos-facade --test corpus_wire_probe -- --ignored --nocapture
#[test]
#[ignore = "work-list probe; run with --ignored --nocapture"]
fn tally_wire_refusals_over_the_corpus() {
    let dir = std::env::var("LOGIFOS_PROBE_DIR").map_or_else(
        |_| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../corpus/reference/plurimath-testsuite/derived")
        },
        std::path::PathBuf::from,
    );
    if !dir.is_dir() {
        eprintln!("probe dir not found ({}); skipping", dir.display());
        return;
    }
    let mut tallies: BTreeMap<String, u64> = BTreeMap::new();
    for entry in std::fs::read_dir(&dir).expect("derived dir reads") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("mml") {
            continue;
        }
        let bytes = std::fs::read(&path).expect("case reads");
        let mut tally = |key: &str| *tallies.entry(key.to_owned()).or_default() += 1;
        let parsed = logifos_syntax_mathml::parse(&bytes);
        match parsed
            .map_err(|error| format!("transform:parse_refused:{:?}", error.kind))
            .and_then(|document| {
                logifos_syntax_mathml::transform(&document).map_err(|error| {
                    format!(
                        "transform:{}:{}:{}",
                        match error.kind {
                            logifos_syntax_mathml::TransformErrorKind::UnsupportedElement =>
                                "element",
                            logifos_syntax_mathml::TransformErrorKind::UnsupportedAttribute =>
                                "attribute",
                            _ => "other",
                        },
                        error.element.local,
                        error
                            .attribute
                            .as_ref()
                            .map(|a| a.local.as_str())
                            .unwrap_or("-")
                    )
                })
            }) {
            Err(kind) => tally(&kind),
            Ok(formula) => match formula.to_wire() {
                Ok(_) => tally("encoded"),
                Err(e) => {
                    let mut root = formula;
                    let kind = classify(&mut root);
                    tally(&format!("wire:{:?} at {} ({kind})", e.code, e.path));
                }
            },
        }
    }
    for (key, count) in &tallies {
        println!("{count:5}  {key}");
    }
}

fn classify(_formula: &mut Formula) -> &'static str {
    "unclassified"
}
