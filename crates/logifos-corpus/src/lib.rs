//! Offline capability grading of the canonical conformance manifest.
//! Refusals are observations, not successful coverage or golden parity.

mod pipeline;
mod reference;

use logifos_layout::font::FontProvider;
use pipeline::{Observation, Outcome};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::{Component, Path},
};

/// Reads a local file within the shared JSON byte ceiling, without unbounded allocation.
pub fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| "cannot open input")?
        .take(logifos_json::MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read input")?;
    if bytes.len() > logifos_json::MAX_BYTES {
        return Err("input byte limit exceeded".into());
    }
    Ok(bytes)
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("missing or invalid {key}"))
}

fn input(root: &Path, case: &Value) -> Result<Vec<u8>, &'static str> {
    let relative = Path::new(case["path"].as_str().ok_or("invalid_path")?);
    let mut path = root.to_path_buf();
    if relative.as_os_str().is_empty() {
        return Err("invalid_path");
    }
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err("invalid_path");
        };
        // Backslashes and drive prefixes must not change meaning on another OS.
        if part.to_string_lossy().contains(['\\', ':']) {
            return Err("invalid_path");
        }
        path.push(part);
        if std::fs::symlink_metadata(&path)
            .map_err(|_| "missing_input")?
            .file_type()
            .is_symlink()
        {
            return Err("symlink_input");
        }
    }
    let bytes = read_bounded(&path).map_err(|_| "unreadable_input")?;
    if bytes.is_empty() {
        return Err("empty_input");
    }
    let expected = case["sha256"].as_str().ok_or("missing_hash")?;
    if format!("{:x}", Sha256::digest(&bytes)) != expected {
        return Err("hash_mismatch");
    }
    Ok(bytes)
}

/// Grades every manifest entry, sorted by stable id. Invalid manifest structure
/// aborts; input integrity errors become failures and do not hide other cases.
/// Supply a hash-verified provider to enable layout and SVG emission. Without
/// one, successful round trips end in an explicit `layout/font_not_configured`
/// refusal. Expected assertions are not an oracle for this capability report.
pub fn grade(manifest_path: &Path, fonts: Option<&mut FontProvider>) -> Result<Value, String> {
    grade_inner(manifest_path, fonts, &mut BTreeMap::new())
}

/// Grades a manifest and captures the emitted SVG of every fully passing
/// case, keyed by case id, for the canon comparison.
pub fn grade_with_captures(
    manifest_path: &Path,
    fonts: Option<&mut FontProvider>,
    captures: &mut BTreeMap<String, String>,
) -> Result<Value, String> {
    grade_inner(manifest_path, fonts, captures)
}

fn grade_inner(
    manifest_path: &Path,
    mut fonts: Option<&mut FontProvider>,
    captures: &mut BTreeMap<String, String>,
) -> Result<Value, String> {
    let manifest_bytes = read_bounded(manifest_path)?;
    let manifest = logifos_json::decode(&manifest_bytes)?;
    if manifest["format_version"] != 1 {
        return Err("unsupported manifest version".into());
    }
    let cases = manifest["cases"]
        .as_array()
        .filter(|c| !c.is_empty())
        .ok_or("empty or invalid cases")?;
    let mut ids = BTreeSet::new();
    let mut entries = Vec::new();
    for case in cases {
        let id = text(case, "id")?;
        if !ids.insert(id) {
            return Err("duplicate case id".into());
        }
        entries.push((id, text(case, "input_kind")?, case));
    }
    entries.sort_by_key(|(id, _, _)| *id);
    let root = manifest_path.parent().ok_or("manifest has no parent")?;
    let mut reports = Vec::new();
    let mut counts = json!({"pass":0,"refusal":0,"failure":0});
    let mut categories = json!({});
    let mut stages = json!({});
    for (id, kind, case) in entries {
        let observations = match input(root, case) {
            Err(category) => vec![Observation::new("input", Outcome::Failure, category)],
            Ok(bytes) => {
                // A pinned case cannot silently use another font.
                let matches = match &case["font_sha256"] {
                    Value::Null => true,
                    Value::String(hash) => fonts.as_deref_mut().is_some_and(|f| {
                        f.primary()
                            .identity
                            .sha256
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<String>()
                            == *hash
                    }),
                    _ => false,
                };
                if !matches {
                    vec![Observation::new(
                        "input",
                        Outcome::Failure,
                        "font_pin_mismatch",
                    )]
                } else {
                    let mut svg = None;
                    let observations =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            pipeline::run_capturing(kind, &bytes, fonts.as_deref_mut(), &mut svg)
                        }))
                        .unwrap_or_else(|_| {
                            svg = None;
                            vec![Observation::new("pipeline", Outcome::Failure, "panic")]
                        });
                    if let Some(last) = observations.last() {
                        if last.outcome == Outcome::Pass {
                            if let Some(svg) = svg {
                                captures.insert(id.to_owned(), svg);
                            }
                        }
                    }
                    observations
                }
            }
        };
        let last = observations.last().ok_or("empty observation")?;
        increment(&mut counts, last.outcome.as_str());
        increment(
            &mut categories,
            &format!("{}:{}:{}", last.outcome.as_str(), last.stage, last.category),
        );
        for observation in &observations {
            increment(
                &mut stages,
                &format!(
                    "{}:{}:{}",
                    observation.stage,
                    observation.outcome.as_str(),
                    observation.category
                ),
            );
        }
        reports.push(json!({"id":id,"input_kind":kind,"input_sha256":case["sha256"],"outcome":last.outcome.as_str(),"stage":last.stage,"category":last.category,"stages":observations.iter().map(Observation::value).collect::<Vec<_>>()}));
    }
    let font = fonts.map(|f| {
        let identity = f.primary().identity.clone();
        json!({"sha256":identity.sha256.iter().map(|b| format!("{b:02x}")).collect::<String>(),"face_index":identity.face_index})
    });
    Ok(
        json!({"format_version":1,"manifest_sha256":format!("{:x}",Sha256::digest(&manifest_bytes)),"font":font,"total":reports.len(),"counts":counts,"categories":categories,"stage_counts":stages,"cases":reports}),
    )
}

fn increment(counts: &mut Value, key: &str) {
    let n = counts[key].as_u64().unwrap_or(0);
    counts[key] = json!(n + 1);
}

/// Stable pretty JSON with a single final newline; no paths or timestamps.
pub fn report_json(report: &Value) -> String {
    format!(
        "{}\n",
        serde_json::to_string_pretty(report).expect("JSON value serializes")
    )
}

/// Replays every fetched reference corpus (see the `reference` module) and
/// returns the aggregate scoreboard.
pub fn replay_reference(reference: &Path) -> Result<Value, String> {
    reference::replay_reference(reference)
}

/// Compares logifos's renders against a reference renderer's artifacts for
/// the shared reference-corpus cases.
pub fn canon_report(reference: &Path, renderer: &str) -> Result<Value, String> {
    reference::canon_report(reference, renderer)
}
