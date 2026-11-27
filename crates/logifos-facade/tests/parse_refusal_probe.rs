use std::{collections::BTreeMap, path::Path};

/// Prints, per corpus case, why `parse` refuses — the MalformedXml work list.
/// Ignored: a reporting tool, not a gate.
#[test]
#[ignore = "work-list probe; run with --ignored --nocapture"]
fn print_parse_refusals_over_the_corpus() {
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
        let name = path
            .file_name()
            .expect("named")
            .to_string_lossy()
            .to_string();
        match logifos_syntax_mathml::parse(&bytes) {
            Err(error) => {
                *tallies.entry(format!("{error:?} :: {name}")).or_default() += 1;
            }
            Ok(document) => {
                if let Err(error) = logifos_syntax_mathml::transform(&document) {
                    *tallies.entry(format!("{error:?} :: {name}")).or_default() += 1;
                }
            }
        }
    }
    for (key, count) in &tallies {
        println!("{count:5}  {key}");
    }
}
