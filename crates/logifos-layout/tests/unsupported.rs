#[allow(dead_code)]
mod support;

use logifos_layout::font::{FontProvider, FontResource};
use logifos_layout::layout::{layout, LayoutError, LayoutOptions};
use logifos_model::Formula;

#[test]
fn space_and_string_literal_refuse_layout_without_visiting_their_operands() {
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
    for node in [Formula::StringLiteral("private".into())] {
        for formula in [
            node.clone(),
            Formula::Sequence(vec![node.clone()]),
            Formula::Table(vec![vec![Formula::Sequence(vec![node])]]),
        ] {
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
