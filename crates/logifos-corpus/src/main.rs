use logifos_corpus::{grade, read_bounded, report_json};
use logifos_layout::font::{FontProvider, FontResource};
use std::path::Path;

const DEFAULT_MANIFEST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../corpus/conformance/manifest.json"
);
const REFERENCE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/reference");
const USAGE: &str = "Usage: logifos-corpus [manifest.json] [--font font.ttf --sha256 SHA256] | --reference | --canon <renderer>\nGrades a manifest (or every fetched reference corpus with --reference) and reports observations as JSON. Exit 0: no failures (refusals allowed); 1: case failures; 2: configuration/manifest error.";

fn run(args: &[String]) -> Result<bool, String> {
    if args.len() == 2 && args[0] == "--canon" {
        let report = logifos_corpus::canon_report(Path::new(REFERENCE_DIR), &args[1])?;
        print!("{}", logifos_corpus::report_json(&report));
        // The canon report is a diff surface; only a harness error refuses.
        return Ok(false);
    }
    if args == ["--reference"] {
        let report = logifos_corpus::replay_reference(Path::new(REFERENCE_DIR))?;
        print!("{}", logifos_corpus::report_json(&report));
        return Ok(report["totals"]["failure"] != 0);
    }
    let (manifest, font) = match args {
        [] => (DEFAULT_MANIFEST, None),
        [manifest] => (manifest.as_str(), None),
        [manifest, option, font, hash_option, hash]
            if option == "--font" && hash_option == "--sha256" =>
        {
            (manifest.as_str(), Some((font, hash)))
        }
        _ => return Err(USAGE.into()),
    };
    let mut provider = if let Some((path, hash)) = font {
        let bytes = read_bounded(Path::new(path))?;
        Some(
            FontProvider::new(
                &[FontResource {
                    id: "grading",
                    sha256: hash,
                    bytes: &bytes,
                    face_index: 0,
                }],
                &["grading"],
            )
            .map_err(|e| format!("font configuration: {}", e.code()))?,
        )
    } else {
        None
    };
    let report = grade(Path::new(manifest), provider.as_mut())?;
    print!("{}", report_json(&report));
    Ok(report["counts"]["failure"] != 0)
}

fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!("{USAGE}");
        return std::process::ExitCode::SUCCESS;
    }
    match run(&args) {
        Ok(false) => std::process::ExitCode::SUCCESS,
        Ok(true) => std::process::ExitCode::from(1),
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::from(2)
        }
    }
}
