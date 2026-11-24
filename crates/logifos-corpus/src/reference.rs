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

use crate::{grade, read_bounded, report_json};

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
    let mut reports = Vec::new();
    let mut totals = json!({"pass":0,"refusal":0,"failure":0});
    let mut fonts = grading_font(reference)?;
    for (id, revision) in &listed {
        let dir = reference.join(id);
        let fetched = std::fs::read_to_string(dir.join(".fetched"))
            .map_err(|_| format!("{id} is not fetched"))?;
        if fetched.trim() != revision {
            return Err(format!(
                "{id} is at {}, pinned to {revision}",
                fetched.trim()
            ));
        }
        let cases = extract(id, &dir)?;
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
        let report = grade(&manifest_path, fonts.as_mut())?;
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
