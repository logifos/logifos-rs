use logifos_corpus::replay_reference;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Reference {
    root: PathBuf,
}

impl Reference {
    fn new(sources: Value) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "logifos-reference-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("sources.json"), sources.to_string()).unwrap();
        Self { root }
    }

    fn corpus(&self, id: &str, revision: &str) -> PathBuf {
        let dir = self.root.join(id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".fetched"), format!("{revision}\n")).unwrap();
        dir
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

fn sources(entries: &[(&str, &str)]) -> Value {
    json!({
        "sources": entries
            .iter()
            .map(|(id, revision)| json!({"id": id, "revision": revision}))
            .collect::<Vec<_>>(),
    })
}

#[test]
fn corpora_replay_through_the_shared_pipeline() {
    let reference = Reference::new(sources(&[
        ("jeuclid", "a1"),
        ("katex", "b2"),
        ("webkit-mathml", "c3"),
    ]));
    let jeuclid = reference.corpus("jeuclid", "a1");
    std::fs::write(
        jeuclid.join("contentmath.mml"),
        "<math xmlns=\"http://www.w3.org/1998/Math/MathML\"><mi>x</mi></math>",
    )
    .unwrap();
    let katex = reference.corpus("katex", "b2");
    std::fs::create_dir_all(katex.join("screenshotter")).unwrap();
    std::fs::write(
        katex.join("screenshotter/ss_data.yaml"),
        "Accents: \\vec{x}\nBlockForm:\n    tex: |\n      x^2\nSkipped:\n    other: 1\n",
    )
    .unwrap();
    let webkit = reference.corpus("webkit-mathml", "c3");
    std::fs::write(
        webkit.join("two-roots.html"),
        "<p><math><mi>a</mi></math></p><p><math><mi>b</mi></math> <math",
    )
    .unwrap();

    let report = replay_reference(reference.path()).unwrap();
    assert_eq!(report["totals"], json!({"pass":0,"refusal":5,"failure":0}));
    let by_id = |id: &str| -> Value {
        report["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == json!(id))
            .unwrap()
            .clone()
    };
    // Sorted by corpus id; each case lands in a typed refusal category.
    let jeuclid = by_id("jeuclid");
    assert_eq!(jeuclid["total"], json!(1));
    assert_eq!(
        jeuclid["categories"]["refusal:layout:font_not_configured"],
        json!(1),
        "the full parse/transform/writer/roundtrip path runs before the font gate"
    );
    let katex = by_id("katex");
    assert_eq!(katex["total"], json!(2), "entries without tex are skipped");
    assert_eq!(
        katex["categories"]["refusal:input:latex_not_yet_supported"],
        json!(2)
    );
    let webkit = by_id("webkit-mathml");
    assert_eq!(webkit["total"], json!(2), "unclosed roots are not cases");
    assert_eq!(
        webkit["categories"]["refusal:parse:unsupported_syntax"],
        json!(2),
        "bare html roots carry no MathML namespace"
    );
    // Derived manifests exist and repeat identically.
    let manifest = std::fs::read(reference.path().join("jeuclid/manifest.json")).unwrap();
    let again = replay_reference(reference.path()).unwrap();
    assert_eq!(report, again);
    let repeated = std::fs::read(reference.path().join("jeuclid/manifest.json")).unwrap();
    assert_eq!(manifest, repeated);
}

#[test]
fn unfetched_or_mismatched_corpora_are_configuration_errors() {
    let reference = Reference::new(sources(&[("jeuclid", "a1")]));
    let error = replay_reference(reference.path()).unwrap_err();
    assert_eq!(error, "jeuclid is not fetched");
    reference.corpus("jeuclid", "stale");
    let error = replay_reference(reference.path()).unwrap_err();
    assert_eq!(error, "jeuclid is at stale, pinned to a1");
}
