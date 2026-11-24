#[allow(dead_code)]
mod support;

use logifos_layout::font::{FontProvider, FontResource};
use logifos_layout::layout::{layout, LayoutError, LayoutOptions};
use logifos_model::Formula;

#[test]
fn syntax_only_limits_refuse_layout_without_visiting_their_operands() {
    let bytes = support::FontBuilder::new(&[('x', 1)], 250, false).finish();
    let digest = support::digest(&bytes);
    let mut fonts = FontProvider::new(
        &[FontResource {
            id: "math",
            bytes: &bytes,
            sha256: &digest,
            face_index: 0,
        }],
        &["math"],
    )
    .unwrap();
    // Missing glyphs must not mask the explicit unsupported-construct path.
    let child = Box::new(Formula::Symbol("private".into()));
    for node in [
        Formula::Under(child.clone(), child.clone()),
        Formula::Over(child.clone(), child.clone()),
        Formula::UnderOver(child.clone(), child.clone(), child),
    ] {
        for formula in [node.clone(), Formula::Sequence(vec![node])] {
            let error = layout(
                &formula,
                &mut fonts,
                LayoutOptions {
                    font_size_px: 18.0,
                    script_level: 0,
                    display: false,
                },
            )
            .unwrap_err();
            assert_eq!(error, LayoutError::NotLaidOutYet);
            assert_eq!(error.code(), "NOT_IMPLEMENTED");
            assert!(!error.to_string().contains("private"));
        }
    }
}
