//! Extraction and replay of the fetched reference-renderer corpora
//! (`corpus/reference/<id>`, pinned in `sources.json`). Every suite is
//! flattened into conformance-manifest cases and graded by the same
//! pipeline: a case passes, refuses (typed capability gaps), or fails
//! (harness/pipeline bugs). Corpus files are external and never commit
//! their bytes here; the generated manifests and the scoreboard are
//! derived artifacts.

use logifos_layout::font::{FontProvider, FontResource};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, ffi::OsStr, path::Path};

use crate::{read_bounded, report_json};

/// One extracted replayable case. `path` is corpus-dir relative; the hash
/// pins the whole source file, which may hold several cases.
struct ExtractedCase {
    id: String,
    kind: &'static str,
    path: String,
    sha256: String,
}

fn file_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Sorted corpus-relative paths with the given extension.
fn walk(dir: &Path, extension: &OsStr) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current).map_err(|_| "unreadable corpus entry")? {
            let entry = entry.map_err(|_| "unreadable corpus entry")?;
            let child = entry.path();
            if entry
                .file_type()
                .map_err(|_| "unreadable corpus entry")?
                .is_dir()
            {
                stack.push(child);
            } else if child.extension() == Some(extension) {
                let relative = child
                    .strip_prefix(dir)
                    .map_err(|_| "unreadable corpus entry")?
                    .to_string_lossy()
                    .replace('\\', "/");
                paths.push(relative);
            }
        }
    }
    paths.sort();
    Ok(paths)
}

/// The KaTeX screenshotter data: each YAML entry is the LaTeX input, either
/// as a plain string or under a `tex` key. The file hash is shared by all
/// of a file's cases.
fn katex_cases(dir: &Path) -> Result<Vec<ExtractedCase>, String> {
    let relative = "screenshotter/ss_data.yaml";
    let bytes = read_bounded(&dir.join(relative)).map_err(|_| "unreadable corpus entry")?;
    let text = std::str::from_utf8(&bytes).map_err(|_| "ss_data.yaml is not utf-8")?;
    let document = yaml_rust2::YamlLoader::load_from_str(text)
        .map_err(|_| "unexpected ss_data.yaml shape")?
        .into_iter()
        .next()
        .ok_or("unexpected ss_data.yaml shape")?;
    let mapping = document.as_hash().ok_or("unexpected ss_data.yaml shape")?;
    let tex = yaml_rust2::Yaml::String("tex".into());
    let mut names = BTreeMap::new();
    for (key, value) in mapping {
        let yaml_rust2::Yaml::String(name) = key else {
            return Err("unexpected ss_data.yaml shape".into());
        };
        let value_tex = matches!(value, yaml_rust2::Yaml::String(_))
            || matches!(
                value.as_hash().and_then(|h| h.get(&tex)),
                Some(yaml_rust2::Yaml::String(_))
            );
        names.insert(name.clone(), value_tex);
    }
    Ok(names
        .into_iter()
        .filter(|(_, has_tex)| *has_tex)
        .map(|(name, _)| ExtractedCase {
            id: format!("ss_data/{name}"),
            kind: "latex",
            path: relative.to_owned(),
            sha256: file_hash(&bytes),
        })
        .collect())
}

/// Byte spans of literal `<math>…</math>` roots inside an HTML document.
/// Scripts that build markup at runtime leave nothing to extract.
fn mathml_spans(bytes: &[u8]) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut from = 0;
    while let Some(start) = find(&bytes[from..], b"<math") {
        let start = from + start;
        let Some(close) = find(&bytes[start..], b"</math>") else {
            break;
        };
        let end = start + close + b"</math>".len();
        spans.push((start, end));
        from = end;
    }
    spans
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|&i| &haystack[i..i + needle.len()] == needle)
}

fn mathml_file_cases(dir: &Path, relative: &str) -> Result<Option<ExtractedCase>, String> {
    let bytes = read_bounded(&dir.join(relative)).map_err(|_| "unreadable corpus entry")?;
    if bytes.is_empty() {
        return Ok(None);
    }
    Ok(Some(ExtractedCase {
        id: relative.to_owned(),
        kind: "mathml",
        path: relative.to_owned(),
        sha256: file_hash(&bytes),
    }))
}

fn html_mathml_cases(dir: &Path, relative: &str) -> Result<Vec<ExtractedCase>, String> {
    let bytes = read_bounded(&dir.join(relative)).map_err(|_| "unreadable corpus entry")?;
    Ok(mathml_spans(&bytes)
        .into_iter()
        .enumerate()
        .map(|(index, _)| ExtractedCase {
            id: format!("{relative}#{index}"),
            kind: "mathml",
            path: relative.to_owned(),
            sha256: file_hash(&bytes),
        })
        .collect())
}

/// One corpus adapter: from the fetched corpus directory to replayable cases.
fn extract(id: &str, dir: &Path) -> Result<Vec<ExtractedCase>, String> {
    match id {
        "katex" => katex_cases(dir),
        "mathjax" | "webkit-mathml" => {
            let mut cases = Vec::new();
            for relative in walk(dir, OsStr::new("html"))? {
                cases.extend(html_mathml_cases(dir, &relative)?);
            }
            Ok(cases)
        }
        "jeuclid" | "lasem" => {
            let mut cases = Vec::new();
            for extension in [OsStr::new("mml"), OsStr::new("xml")] {
                for relative in walk(dir, extension)? {
                    cases.extend(mathml_file_cases(dir, &relative)?);
                }
            }
            Ok(cases)
        }
        other => Err(format!("unknown corpus id {other}")),
    }
}

/// Builds the grading-font provider from the fetched `corpus/fonts.json`
/// entries: the recorded revision must match `.fetched` and the file bytes
/// must match the pinned hash. No fetched font grades without one; cases
/// then stop at the layout gate exactly as before.
fn grading_font(parent: &Path) -> Result<Option<FontProvider>, String> {
    let Some(parent) = parent.parent() else {
        return Ok(None);
    };
    let Ok(listing) = read_bounded(&parent.join("fonts.json")) else {
        return Ok(None);
    };
    let listing = logifos_json::decode(&listing)?;
    let mut provider = None;
    for font in listing["fonts"].as_array().ok_or("invalid fonts.json")? {
        let id = font["id"].as_str().ok_or("invalid fonts.json")?;
        let revision = font["revision"]
            .as_str()
            .ok_or("invalid fonts.json")?
            .to_owned();
        let expected = font["sha256"].as_str().ok_or("invalid fonts.json")?;
        let file = font["path"]
            .as_str()
            .and_then(|p| Path::new(p).file_name())
            .ok_or("invalid fonts.json")?
            .to_string_lossy()
            .into_owned();
        let dir = parent.join("fonts").join(id);
        let fetched = std::fs::read_to_string(dir.join(".fetched"))
            .map_err(|_| format!("{id} font is not fetched"))?;
        if fetched.trim() != revision {
            return Err(format!(
                "{id} font is at {}, pinned to {revision}",
                fetched.trim()
            ));
        }
        let bytes =
            read_bounded(&dir.join(&file)).map_err(|_| format!("{id} font is unreadable"))?;
        if format!("{:x}", Sha256::digest(&bytes)) != expected {
            return Err(format!("{id} font bytes do not match the pinned hash"));
        }
        let sha = expected.to_owned();
        let id = id.to_owned();
        provider.get_or_insert_with(|| {
            FontProvider::new(
                &[FontResource {
                    id: &id,
                    sha256: &sha,
                    bytes: &bytes,
                    face_index: 0,
                }],
                &[&id],
            )
            .expect("a hash-verified font always builds a provider")
        });
    }
    Ok(provider)
}

/// Grades every fetched corpus and returns the aggregate scoreboard. The
/// `sources.json` revision and the corpus's recorded `.fetched` revision
/// must agree; manifests and cases derive deterministically from the
/// fetched bytes.
pub fn replay_reference(reference: &Path) -> Result<Value, String> {
    let listed = listed_corpora(reference)?;
    let mut reports = Vec::new();
    let mut totals = json!({"pass":0,"refusal":0,"failure":0});
    let mut fonts = grading_font(reference)?;
    for (id, revision) in &listed {
        let dir = reference.join(id);
        let mut captures = BTreeMap::new();
        let report = run_corpus(id, revision, &dir, fonts.as_mut(), &mut captures)?;
        for key in ["pass", "refusal", "failure"] {
            let n = totals[key].as_u64().unwrap_or(0);
            totals[key] = json!(n + report["counts"][key].as_u64().unwrap_or(0));
        }
        reports.push(json!({
            "id": id,
            "revision": revision,
            "total": report["total"],
            "counts": report["counts"],
            "categories": report["categories"],
            "stage_counts": report["stage_counts"],
            "font": report["font"],
        }));
    }
    Ok(json!({
        "format_version": 1,
        "totals": totals,
        "sources": reports,
    }))
}

/// The pinned corpus id/revision pairs from `sources.json`, sorted by id.
fn listed_corpora(reference: &Path) -> Result<BTreeMap<String, String>, String> {
    let sources = read_bounded(&reference.join("sources.json"))?;
    let manifest_source = logifos_json::decode(&sources)?;
    let mut listed = BTreeMap::new();
    for source in manifest_source["sources"]
        .as_array()
        .ok_or("invalid sources.json")?
    {
        let id = source["id"].as_str().ok_or("invalid sources.json")?;
        let revision = source["revision"]
            .as_str()
            .ok_or("invalid sources.json")?
            .to_owned();
        if listed.insert(id.to_owned(), revision).is_some() {
            return Err("duplicate corpus id".into());
        }
    }
    Ok(listed)
}

/// Verifies the fetched revision, regenerates the manifest, and grades one
/// corpus; `captures` receives the emitted SVG of every fully passing case.
fn run_corpus(
    id: &str,
    revision: &str,
    dir: &Path,
    fonts: Option<&mut FontProvider>,
    captures: &mut BTreeMap<String, String>,
) -> Result<Value, String> {
    let fetched = std::fs::read_to_string(dir.join(".fetched"))
        .map_err(|_| format!("{id} is not fetched"))?;
    if fetched.trim() != revision {
        return Err(format!(
            "{id} is at {}, pinned to {revision}",
            fetched.trim()
        ));
    }
    let cases = extract(id, dir)?;
    let manifest = json!({
        "format_version": 1,
        "cases": cases
            .iter()
            .map(|case| json!({
                "id": case.id,
                "input_kind": case.kind,
                "path": case.path,
                "sha256": case.sha256,
                "font_sha256": Value::Null,
            }))
            .collect::<Vec<_>>(),
    });
    let manifest_path = dir.join("manifest.json");
    std::fs::write(&manifest_path, report_json(&manifest))
        .map_err(|_| format!("{id} manifest is unwritable"))?;
    crate::grade_with_captures(&manifest_path, fonts, captures)
}

/// Structural record of an SVG: element counts and root attributes, read by
/// byte scan so the comparison needs no XML parser.
fn svg_record(bytes: &[u8]) -> Value {
    let count = |needle: &[u8]| -> u64 {
        let mut hits = 0u64;
        let mut from = 0;
        while let Some(at) = find(&bytes[from..], needle) {
            hits += 1;
            from += at + 1;
        }
        hits
    };
    json!({
        "bytes": bytes.len(),
        "path": count(b"<path"),
        "rect": count(b"<rect"),
        "use": count(b"<use"),
        "g": count(b"<g"),
        "defs": count(b"<defs"),
        "view_box": bytes.windows(7).any(|window| window == b"viewBox"),
    })
}

/// The corpus-relative artifact name for a case id, matching the renderer
/// script's flattening.
fn artifact_name(corpus: &str, case_id: &str) -> String {
    format!("{corpus}__{}", case_id.replace('/', "__"))
}

/// Structural comparison of logifos's renders against a reference
/// renderer's artifacts for the shared cases. The pin at
/// `canon/<renderer>.json` records the renderer version; artifacts live in
/// `canon/<renderer>/` as `.svg` (or `.error` for cases the reference
/// could not render). Cases logifos does not fully render stay out of the
/// shared set, and the aggregate counts carry the pass-rate gate.
pub fn canon_report(reference: &Path, renderer: &str) -> Result<Value, String> {
    let parent = reference.parent().ok_or("reference has no parent")?;
    let pin = read_bounded(&parent.join("canon").join(format!("{renderer}.json")))?;
    let pin = logifos_json::decode(&pin)?;
    let version = pin["version"]
        .as_str()
        .or_else(|| pin["revision"].as_str())
        .ok_or("invalid renderer pin")?;
    let artifacts = parent.join("canon").join(renderer);
    let listed = listed_corpora(reference)?;
    let mut fonts = grading_font(reference)?;
    let mut shared = Vec::new();
    let mut logifos_renders = 0u64;
    let mut reference_renders = 0u64;
    let mut reference_errors = 0u64;
    let mut total = 0u64;
    for corpus in listed.keys() {
        let dir = reference.join(corpus);
        let mut captures = BTreeMap::new();
        let report = run_corpus(corpus, &listed[corpus], &dir, fonts.as_mut(), &mut captures)?;
        total += report["total"].as_u64().unwrap_or(0);
        logifos_renders += report["counts"]["pass"].as_u64().unwrap_or(0);
        for (case_id, svg) in captures {
            let name = artifact_name(corpus, &case_id);
            let ours = svg_record(svg.as_bytes());
            let svg_path = artifacts.join(format!("{name}.svg"));
            let error_path = artifacts.join(format!("{name}.error"));
            let theirs = if let Ok(bytes) = read_bounded(&svg_path) {
                reference_renders += 1;
                svg_record(&bytes)
            } else if let Ok(bytes) = read_bounded(&error_path) {
                reference_errors += 1;
                json!({"error": String::from_utf8_lossy(bytes.trim_ascii()).into_owned()})
            } else {
                continue;
            };
            shared.push(json!({
                "corpus": corpus,
                "id": case_id,
                "logifos": ours,
                renderer: theirs,
            }));
        }
    }
    let matching = shared
        .iter()
        .filter(|case| {
            let mut ours = case["logifos"].clone();
            ours["bytes"] = json!(0);
            let mut theirs = case[renderer].clone();
            theirs["bytes"] = json!(0);
            ours == theirs
        })
        .count();
    Ok(json!({
        "format_version": 1,
        "renderer": renderer,
        "version": version,
        "total": total,
        "logifos_renders": logifos_renders,
        "reference_renders": reference_renders,
        "reference_errors": reference_errors,
        "shared": shared.len(),
        "matching_structure": matching,
        "cases": shared,
    }))
}
