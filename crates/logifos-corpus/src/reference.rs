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

/// The plurimath-testsuite mathml stage: YAML payload rows whose `input`
/// strings are the graded MathML. Each row materializes as a derived file
/// under `derived/` (the corpus directory is gitignored), so the manifest
/// stays in the file-per-case shape the grader consumes.
fn testsuite_cases(dir: &Path) -> Result<Vec<ExtractedCase>, String> {
    let mut payloads = walk(dir, OsStr::new("yaml"))?;
    payloads.retain(|p| p.contains("payloads/"));
    let mut cases = Vec::new();
    let derived = dir.join("derived");
    std::fs::create_dir_all(&derived).map_err(|_| "derived dir is unwritable")?;
    for relative in payloads {
        let bytes = read_bounded(&dir.join(&relative)).map_err(|_| "unreadable corpus entry")?;
        let text = std::str::from_utf8(&bytes).map_err(|_| "payload is not utf-8")?;
        let document = yaml_rust2::YamlLoader::load_from_str(text)
            .map_err(|_| "unexpected payload shape")?
            .into_iter()
            .next()
            .ok_or("unexpected payload shape")?;
        let rows = document
            .as_hash()
            .ok_or("unexpected payload shape")?
            .get(&yaml_rust2::Yaml::String("cases".into()))
            .and_then(|cases| cases.as_vec())
            .ok_or("unexpected payload shape")?;
        // generations/000003/payloads/fences.yaml from the walk path; the
        // generation number is the directory before payloads.
        let generation = relative
            .split('/')
            .take_while(|part| *part != "payloads")
            .last()
            .unwrap_or("generations")
            .to_owned();
        let payload = Path::new(&relative)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        for row in rows {
            let entry = row
                .as_hash()
                .and_then(|h| h.get(&yaml_rust2::Yaml::String("input".into())))
                .ok_or("unexpected payload shape")?;
            // Plain rows carry a string; model-roundtrip rows wrap the
            // MathML in a `str` mapping.
            let input = entry.as_str().map(str::to_owned).or_else(|| {
                entry
                    .as_hash()
                    .and_then(|h| h.get(&yaml_rust2::Yaml::String("str".into())))
                    .and_then(|str| str.as_str().map(str::to_owned))
            });
            let input = match input {
                Some(input) => input,
                None => continue,
            };
            let id = row
                .as_hash()
                .and_then(|h| h.get(&yaml_rust2::Yaml::String("id".into())))
                .and_then(|id| id.as_str().map(str::to_owned))
                .ok_or("unexpected payload shape")?;
            let name = format!("{generation}__{payload}__{id}.mml");
            let path = format!("derived/{name}");
            std::fs::write(derived.join(&name), &input)
                .map_err(|_| "derived case is unwritable")?;
            let written = read_bounded(&dir.join(&path)).map_err(|_| "unreadable corpus entry")?;
            cases.push(ExtractedCase {
                id: format!("{generation}/{payload}/{id}"),
                kind: "mathml",
                path,
                sha256: file_hash(&written),
            });
        }
    }
    Ok(cases)
}

/// One corpus adapter: from the fetched corpus directory to replayable cases.
fn extract(id: &str, dir: &Path) -> Result<Vec<ExtractedCase>, String> {
    match id {
        "katex" => katex_cases(dir),
        "plurimath-testsuite" => testsuite_cases(dir),
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

/// One verified, fetched grading font: its provider plus its pinned hash.
struct GradingFont {
    id: String,
    sha256: String,
    provider: FontProvider,
}

/// Builds one verified provider per fetched `corpus/fonts.json` entry: the
/// file bytes must match the pinned hash, and a git-sourced entry's
/// recorded revision must match its `.fetched` marker. Fontist-sourced
/// faces have no fetch marker; their hash is the pin. No fetched font
/// grades without verification; an empty list keeps cases at the layout
/// gate exactly as before.
fn grading_fonts(parent: &Path) -> Result<Vec<GradingFont>, String> {
    let Some(parent) = parent.parent() else {
        return Ok(Vec::new());
    };
    let Ok(listing) = read_bounded(&parent.join("fonts.json")) else {
        return Ok(Vec::new());
    };
    let listing = logifos_json::decode(&listing)?;
    let mut fonts = Vec::new();
    for font in listing["fonts"].as_array().ok_or("invalid fonts.json")? {
        let id = font["id"].as_str().ok_or("invalid fonts.json")?;
        let expected = font["sha256"].as_str().ok_or("invalid fonts.json")?;
        let face_index = u16::try_from(font["face_index"].as_u64().unwrap_or(0))
            .map_err(|_| "invalid fonts.json face_index")?;
        let file = font["path"]
            .as_str()
            .and_then(|p| Path::new(p).file_name())
            .ok_or("invalid fonts.json")?
            .to_string_lossy()
            .into_owned();
        let dir = parent.join("fonts").join(id);
        if let Some(revision) = font["revision"].as_str() {
            let recorded = std::fs::read_to_string(dir.join(".fetched"))
                .map_err(|_| format!("{id} font is not fetched"))?;
            if recorded.trim() != revision {
                return Err(format!(
                    "{id} font is at {}, pinned to {revision}",
                    recorded.trim()
                ));
            }
        }
        let bytes =
            read_bounded(&dir.join(&file)).map_err(|_| format!("{id} font is unreadable"))?;
        if format!("{:x}", Sha256::digest(&bytes)) != expected {
            return Err(format!("{id} font bytes do not match the pinned hash"));
        }
        let sha = expected.to_owned();
        let id = id.to_owned();
        let provider = FontProvider::new(
            &[FontResource {
                id: &id,
                sha256: &sha,
                bytes: &bytes,
                face_index,
            }],
            &[&id],
        )
        .expect("a hash-verified font always builds a provider");
        fonts.push(GradingFont {
            id,
            sha256: sha,
            provider,
        });
    }
    fonts.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(fonts)
}

/// Grades every fetched corpus and returns the aggregate scoreboard, one
/// entry per corpus per grading font: glyph metrics and renders differ per
/// face, so outcomes are recorded for each. Without any fetched font, each
/// corpus is graded once and stops at the layout gate. The `sources.json`
/// revision and the corpus's recorded `.fetched` revision must agree;
/// manifests and cases derive deterministically from the fetched bytes.
pub fn replay_reference(reference: &Path) -> Result<Value, String> {
    let listed = listed_corpora(reference)?;
    let mut grading = grading_fonts(reference)?;
    let mut reports = Vec::new();
    let mut totals = json!({"pass":0,"refusal":0,"failure":0});
    for (id, revision) in &listed {
        let dir = reference.join(id);
        let loops: Vec<Option<&mut GradingFont>> = if grading.is_empty() {
            vec![None]
        } else {
            grading.iter_mut().map(Some).collect()
        };
        for mut font in loops {
            let mut captures = BTreeMap::new();
            let meta = font
                .as_ref()
                .map(|f| json!({"id": f.id, "sha256": f.sha256}));
            let provider = font.as_mut().map(|f| &mut f.provider);
            let report = run_corpus(id, revision, &dir, provider, &mut captures)?;
            for key in ["pass", "refusal", "failure"] {
                let n = totals[key].as_u64().unwrap_or(0);
                totals[key] = json!(n + report["counts"][key].as_u64().unwrap_or(0));
            }
            reports.push(json!({
                "id": id,
                "revision": revision,
                "font": meta,
                "total": report["total"],
                "counts": report["counts"],
                "categories": report["categories"],
                "stage_counts": report["stage_counts"],
            }));
        }
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
/// corpus; `captures` receives the outcomes of every fully passing case.
fn run_corpus(
    id: &str,
    revision: &str,
    dir: &Path,
    fonts: Option<&mut FontProvider>,
    captures: &mut BTreeMap<String, crate::Capture>,
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

/// Materializes the desired render of every fully passing reference-corpus
/// case for every fetched grading font: `<out>/<font>/<corpus>/<case>.svg`
/// plus a checksums manifest whose `metrics` records each case's laid-out
/// width/height/depth per font — the SVG is one outcome, not the only one.
/// Deterministic per pinned corpus revisions and engine version.
pub fn emit_desired(reference: &Path, out: &Path) -> Result<Value, String> {
    let listed = listed_corpora(reference)?;
    let mut grading = grading_fonts(reference)?;
    let engine = env!("CARGO_PKG_VERSION");
    std::fs::create_dir_all(out).map_err(|_| "desired dir is unwritable")?;
    let mut written = 0u64;
    let mut checksums = serde_json::Map::new();
    let mut metrics = serde_json::Map::new();
    for font in &mut grading {
        for (id, revision) in &listed {
            let dir = reference.join(id);
            let mut captures = BTreeMap::new();
            run_corpus(id, revision, &dir, Some(&mut font.provider), &mut captures)?;
            let corpus_dir = out.join(&font.id).join(id);
            std::fs::create_dir_all(&corpus_dir).map_err(|_| "desired corpus dir is unwritable")?;
            let mut case_ids = captures.keys().cloned().collect::<Vec<_>>();
            case_ids.sort();
            for case_id in case_ids {
                let capture = &captures[&case_id];
                let name = format!("{}.svg", case_id.replace('/', "__"));
                let relative = format!("{}/{}/{}", font.id, id, name);
                std::fs::write(corpus_dir.join(&name), &capture.svg)
                    .map_err(|_| "desired render is unwritable")?;
                checksums.insert(
                    relative.clone(),
                    json!(format!("{:x}", Sha256::digest(capture.svg.as_bytes()))),
                );
                let (width, height, depth) = capture.metrics;
                metrics.insert(relative, json!([width, height, depth]));
                written += 1;
            }
        }
    }
    Ok(json!({
        "format_version": 2,
        "engine_version": engine,
        "fonts": grading
            .iter()
            .map(|f| json!({"id": f.id, "sha256": f.sha256}))
            .collect::<Vec<_>>(),
        "written": written,
        "sha256": checksums,
        "metrics": metrics,
    }))
}

/// Structural comparison of logifos's renders against a reference
/// renderer's artifacts for the shared cases, per grading font. The pin at
/// `canon/<renderer>.json` records the renderer version; artifacts live in
/// `canon/<renderer>/` as `.svg` (or `.error` for cases the reference
/// could not render), while ours land in `canon/logifos/<font>/`. Cases
/// logifos does not fully render with a font stay out of that font's
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
    let ours = parent.join("canon").join("logifos");
    let listed = listed_corpora(reference)?;
    let mut grading = grading_fonts(reference)?;
    let mut shared = Vec::new();
    let mut logifos_renders = 0u64;
    let mut reference_renders = 0u64;
    let mut reference_errors = 0u64;
    let mut total = 0u64;
    for font in &mut grading {
        for corpus in listed.keys() {
            let dir = reference.join(corpus);
            let mut captures = BTreeMap::new();
            let report = run_corpus(
                corpus,
                &listed[corpus],
                &dir,
                Some(&mut font.provider),
                &mut captures,
            )?;
            total += report["total"].as_u64().unwrap_or(0);
            logifos_renders += report["counts"]["pass"].as_u64().unwrap_or(0);
            // The emitted renders persist under canon/logifos/<font>/ so the
            // visible review set pairs them with the reference artifacts.
            let font_dir = ours.join(&font.id);
            std::fs::create_dir_all(&font_dir).map_err(|_| "logifos canon dir is unwritable")?;
            for (case_id, capture) in &captures {
                let name = artifact_name(corpus, case_id);
                std::fs::write(font_dir.join(format!("{name}.svg")), &capture.svg)
                    .map_err(|_| "logifos canon artifact is unwritable")?;
                let ours = svg_record(capture.svg.as_bytes());
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
                    "font": font.id,
                    "corpus": corpus,
                    "id": case_id,
                    "logifos": ours,
                    renderer: theirs,
                }));
            }
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
