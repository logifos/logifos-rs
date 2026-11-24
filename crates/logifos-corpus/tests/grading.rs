#[allow(dead_code)]
#[path = "../../logifos-layout/tests/support/mod.rs"]
mod font_support;

use logifos_corpus::{grade, report_json};
use logifos_layout::font::{FontProvider, FontResource};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

fn canonical() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/conformance/manifest.json")
}

struct Corpus {
    root: PathBuf,
    manifest: Value,
}
impl Corpus {
    fn new(inputs: &[(&str, &[u8])]) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "logifos-corpus-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let cases: Vec<_> = inputs.iter().map(|(id, bytes)| {
            let path = format!("{id}.mml");
            std::fs::write(root.join(&path), bytes).unwrap();
            json!({"id":id,"path":path,"input_kind":"mathml","sha256":format!("{:x}",Sha256::digest(bytes)),"font_sha256":null})
        }).collect();
        let corpus = Self {
            root,
            manifest: json!({"format_version":1,"cases":cases}),
        };
        corpus.save();
        corpus
    }
    fn save(&self) {
        std::fs::write(self.path(), self.manifest.to_string()).unwrap();
    }
    fn path(&self) -> PathBuf {
        self.root.join("manifest.json")
    }
}
impl Drop for Corpus {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn canonical_report_matches_snapshot_and_cli() {
    let report = grade(&canonical(), None).unwrap();
    let bytes = report_json(&report);
    assert_eq!(bytes, include_str!("conformance-report.json"));
    assert_eq!(bytes, report_json(&grade(&canonical(), None).unwrap()));
    assert_eq!(report["total"], 6);
    assert_eq!(report["counts"], json!({"pass":0,"refusal":6,"failure":0}));
    let cli = Command::new(env!("CARGO_BIN_EXE_logifos-corpus"))
        .output()
        .unwrap();
    assert_eq!(cli.status.code(), Some(0));
    assert_eq!(cli.stdout, bytes.as_bytes());
}

#[test]
fn typed_refusals_and_integrity_failures_are_counted_without_skips() {
    let mut corpus = Corpus::new(&[
        ("ok", b"<math><mi>x</mi></math>"),
        ("bad-xml", b"<math>"),
        ("unsafe", b"<!DOCTYPE math><math/>"),
        ("style", b"<math mathcolor='red'/>"),
        ("enclosure", b"<math><menclose/></math>"),
        ("broken", b"<math/>"),
    ]);
    corpus.manifest["cases"][5]["sha256"] = json!("0".repeat(64));
    corpus.save();
    let report = grade(&corpus.path(), None).unwrap();
    assert_eq!(report["total"], 6);
    assert_eq!(report["counts"], json!({"pass":0,"refusal":5,"failure":1}));
    for (category, count) in [
        ("refusal:parse:malformed_xml", 1),
        ("refusal:parse:unsafe_xml", 1),
        ("refusal:transform:unsupported_attribute", 1),
        ("refusal:transform:unsupported_element", 1),
        ("refusal:layout:font_not_configured", 1),
        ("failure:input:hash_mismatch", 1),
    ] {
        assert_eq!(report["categories"][category], count);
    }
    let cli = Command::new(env!("CARGO_BIN_EXE_logifos-corpus"))
        .arg(corpus.path())
        .output()
        .unwrap();
    assert_eq!(cli.status.code(), Some(1));
    let cli_report = logifos_json::decode(&cli.stdout).unwrap();
    assert_eq!(cli_report, report);
}

#[test]
fn synthetic_font_exercises_layout_emit_and_missing_glyph_refusal() {
    let bytes = font_support::FontBuilder::new(&[('x', 1)], 250, false).finish();
    let hash = font_support::digest(&bytes);
    let mut fonts = FontProvider::new(
        &[FontResource {
            id: "test",
            sha256: &hash,
            bytes: &bytes,
            face_index: 0,
        }],
        &["test"],
    )
    .unwrap();
    let mut corpus = Corpus::new(&[
        ("x", b"<math><mi>x</mi></math>"),
        ("missing", b"<math><mi>q</mi></math>"),
    ]);
    corpus.manifest["cases"][0]["font_sha256"] = json!(hash);
    corpus.save();
    let report = grade(&corpus.path(), Some(&mut fonts)).unwrap();
    assert_eq!(report["counts"], json!({"pass":1,"refusal":1,"failure":0}));
    assert_eq!(report["categories"]["refusal:layout:MISSING_GLYPH"], 1);
    assert_eq!(report["stage_counts"]["emit:pass:complete"], 1);
    assert_eq!(report["font"]["sha256"], hash);
    corpus.manifest["cases"][0]["font_sha256"] = json!("0".repeat(64));
    corpus.save();
    assert_eq!(
        grade(&corpus.path(), Some(&mut fonts)).unwrap()["categories"]
            ["failure:input:font_pin_mismatch"],
        1
    );
    let font_path = corpus.root.join("font.ttf");
    std::fs::write(&font_path, &bytes).unwrap();
    let cli = Command::new(env!("CARGO_BIN_EXE_logifos-corpus"))
        .arg(corpus.path())
        .arg("--font")
        .arg(font_path)
        .arg("--sha256")
        .arg("0".repeat(64))
        .output()
        .unwrap();
    assert_eq!(cli.status.code(), Some(2));
}

#[test]
fn manifest_order_cannot_change_case_order_or_counts() {
    let mut corpus = Corpus::new(&[("z", b"<math/>"), ("a", b"<math/>")]);
    let before = grade(&corpus.path(), None).unwrap();
    corpus.manifest["cases"].as_array_mut().unwrap().reverse();
    corpus.save();
    let after = grade(&corpus.path(), None).unwrap();
    for field in ["cases", "counts", "categories", "stage_counts"] {
        assert_eq!(before[field], after[field]);
    }
    assert_eq!(after["cases"][0]["id"], "a");
}

#[test]
fn malformed_manifests_abort_and_bad_inputs_fail_closed() {
    let mut corpus = Corpus::new(&[("case", b"<math/>")]);
    for path in ["../outside", "/absolute", "C:\\outside", ""] {
        corpus.manifest["cases"][0]["path"] = json!(path);
        corpus.save();
        assert_eq!(
            grade(&corpus.path(), None).unwrap()["categories"]["failure:input:invalid_path"],
            1
        );
    }
    corpus.manifest["cases"][0]["path"] = json!("missing.mml");
    corpus.save();
    assert_eq!(
        grade(&corpus.path(), None).unwrap()["categories"]["failure:input:missing_input"],
        1
    );
    corpus.manifest["cases"][0]["path"] = json!("case.mml");
    corpus.manifest["cases"][0]["input_kind"] = json!("future");
    corpus.save();
    assert_eq!(
        grade(&corpus.path(), None).unwrap()["categories"]["failure:input:unsupported_input_kind"],
        1
    );
    corpus.manifest["format_version"] = json!(2);
    corpus.save();
    assert!(grade(&corpus.path(), None).is_err());
    corpus.manifest["format_version"] = json!(1);
    let duplicate = corpus.manifest["cases"][0].clone();
    corpus.manifest["cases"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    corpus.save();
    assert!(grade(&corpus.path(), None).is_err());
    corpus.manifest["cases"] = json!([]);
    corpus.save();
    assert!(grade(&corpus.path(), None).is_err());
    let cli = Command::new(env!("CARGO_BIN_EXE_logifos-corpus"))
        .arg(corpus.path())
        .output()
        .unwrap();
    assert_eq!(cli.status.code(), Some(2));
}

#[cfg(unix)]
#[test]
fn symlink_inputs_are_failures() {
    let mut corpus = Corpus::new(&[("case", b"<math/>")]);
    std::os::unix::fs::symlink("case.mml", corpus.root.join("link.mml")).unwrap();
    corpus.manifest["cases"][0]["path"] = json!("link.mml");
    corpus.save();
    assert_eq!(
        grade(&corpus.path(), None).unwrap()["categories"]["failure:input:symlink_input"],
        1
    );
}

#[test]
fn writer_and_parser_resource_refusals_keep_distinct_stages() {
    let expansion = format!("<math><mi>{}</mi></math>", ">".repeat(300_000));
    let excessive = vec![b'x'; 1_048_577];
    let corpus = Corpus::new(&[("expansion", expansion.as_bytes()), ("large", &excessive)]);
    let report = grade(&corpus.path(), None).unwrap();
    assert_eq!(report["counts"], json!({"pass":0,"refusal":2,"failure":0}));
    assert_eq!(report["categories"]["refusal:writer:resource_limit"], 1);
    assert_eq!(report["categories"]["refusal:parse:resource_limit"], 1);
    assert!(report["stage_counts"]["roundtrip:pass:complete"].is_null());
}

#[test]
fn canonical_corpus_reaches_svg_with_a_pinned_font() {
    let bytes = font_support::FontBuilder::new(&[('q', 1), ('z', 1)], 250, false).finish();
    let hash = font_support::digest(&bytes);
    let mut fonts = FontProvider::new(
        &[FontResource {
            id: "test",
            sha256: &hash,
            bytes: &bytes,
            face_index: 0,
        }],
        &["test"],
    )
    .unwrap();
    let report = grade(&canonical(), Some(&mut fonts)).unwrap();
    assert_eq!(report["counts"], json!({"pass":2,"refusal":4,"failure":0}));
    assert_eq!(report["stage_counts"]["layout:pass:complete"], 2);
    assert_eq!(report["stage_counts"]["emit:pass:complete"], 2);
    assert_eq!(report, grade(&canonical(), Some(&mut fonts)).unwrap());
    let corpus = Corpus::new(&[("holder", b"<math/>")]);
    let font_path = corpus.root.join("font.ttf");
    std::fs::write(&font_path, &bytes).unwrap();
    let cli = Command::new(env!("CARGO_BIN_EXE_logifos-corpus"))
        .arg(canonical())
        .arg("--font")
        .arg(font_path)
        .arg("--sha256")
        .arg(hash)
        .output()
        .unwrap();
    assert_eq!(cli.status.code(), Some(0));
    assert_eq!(logifos_json::decode(&cli.stdout).unwrap(), report);
}

#[test]
fn presentation_containers_complete_the_grading_pipeline() {
    let bytes = font_support::FontBuilder::new(&[('x', 1)], 250, false).finish();
    let hash = font_support::digest(&bytes);
    let mut fonts = FontProvider::new(
        &[FontResource {
            id: "test",
            sha256: &hash,
            bytes: &bytes,
            face_index: 0,
        }],
        &["test"],
    )
    .unwrap();
    let corpus = Corpus::new(&[
        ("style", b"<math><mstyle><mi>x</mi></mstyle></math>"),
        ("error", b"<math><merror><mi>x</mi></merror></math>"),
        ("padding", b"<math><mpadded><mi>x</mi></mpadded></math>"),
        ("phantom", b"<math><mphantom><mfrac><mi>x</mi><mi>x</mi></mfrac></mphantom></math>"),
        ("semantics", b"<math><semantics><mi>x</mi><annotation encoding='text/plain'>meaning</annotation></semantics></math>"),
        ("missing", b"<math><semantics/></math>"),
        ("annotation-first", b"<math><semantics><annotation/><mi>x</mi></semantics></math>"),
        ("attributes", b"<math><mpadded width='2em'><mi>x</mi></mpadded></math>"),
        ("hidden-missing-glyph", b"<math><mphantom><mi>q</mi></mphantom></math>"),
    ]);
    let report = grade(&corpus.path(), Some(&mut fonts)).unwrap();
    assert_eq!(report["counts"], json!({"pass":5,"refusal":4,"failure":0}));
    assert_eq!(report["stage_counts"]["emit:pass:complete"], 5);
    assert_eq!(
        report["categories"]["refusal:transform:invalid_structure"],
        2
    );
    assert_eq!(
        report["categories"]["refusal:transform:unsupported_attribute"],
        1
    );
    assert_eq!(report["categories"]["refusal:layout:MISSING_GLYPH"], 1);
}
