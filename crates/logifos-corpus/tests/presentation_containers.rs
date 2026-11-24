#[allow(dead_code)]
#[path = "../../logifos-layout/tests/support/mod.rs"]
mod support;

use logifos_layout::{
    font::{FontProvider, FontResource},
    layout::{layout, Layout, LayoutError, LayoutOptions},
};
use logifos_svg::{emit, SvgOptions};
use logifos_syntax_mathml::{parse, transform};

fn render(body: &str) -> Result<Layout, LayoutError> {
    let bytes = support::FontBuilder::new(
        &[('x', 1), ('y', 1), ('+', 1), ('(', 2), (')', 2)],
        250,
        true,
    )
    .finish();
    let hash = support::digest(&bytes);
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
    let formula = transform(&parse(format!("<math>{body}</math>")).unwrap()).unwrap();
    layout(
        &formula,
        &mut fonts,
        LayoutOptions {
            font_size_px: 18.0,
            script_level: 0,
            display: false,
        },
    )
}

fn svg(layout: &Layout) -> String {
    emit(&layout.tree, &layout.glyphs, &SvgOptions::default())
        .unwrap()
        .as_str()
        .to_owned()
}

#[test]
fn visible_containers_have_row_layout_and_svg() {
    for content in [
        "",
        "<mi>x</mi>",
        "<mi>x</mi><mo>+</mo><mi>y</mi>",
        "<mfrac><mi>x</mi><mi>y</mi></mfrac>",
    ] {
        let row = render(&format!("<mrow>{content}</mrow>")).unwrap();
        for name in ["mstyle", "merror", "mpadded"] {
            let grouped = render(&format!("<{name}>{content}</{name}>")).unwrap();
            assert_eq!(grouped.tree, row.tree, "{name}");
            assert_eq!(grouped.glyphs, row.glyphs, "{name}");
            assert_eq!(svg(&grouped), svg(&row), "{name}");
        }
    }
}

#[test]
fn phantom_retains_dimensions_without_glyph_or_rule_output() {
    let content = "<mfrac><mi>x</mi><mi>y</mi></mfrac>";
    let visible = render(&format!("<mrow>{content}</mrow>")).unwrap();
    assert!(visible.tree.ink_bounds().is_some());
    assert!(!visible.glyphs.is_empty());
    assert!(svg(&visible).contains("<rect"));
    for name in ["mphantom", "mstyle"] {
        let hidden = render(&format!("<{name}><mphantom>{content}</mphantom></{name}>")).unwrap();
        assert_eq!(hidden.tree.metrics(), visible.tree.metrics());
        assert!(hidden.tree.ink_bounds().is_none());
        assert!(hidden.glyphs.is_empty());
        let output = svg(&hidden);
        assert!(!output.contains("<path"));
        assert!(!output.contains("<use"));
        assert!(!output.contains("<rect"));
    }
    let empty = render("<mphantom/>").unwrap();
    assert_eq!(empty.tree.metrics().width(), 0.0);
    assert!(empty.tree.ink_bounds().is_none());
}

#[test]
fn phantom_reserves_space_between_visible_siblings() {
    let visible =
        render("<mi>x</mi><mrow><mfrac><mi>x</mi><mi>y</mi></mfrac></mrow><mi>y</mi>").unwrap();
    let hidden =
        render("<mi>x</mi><mphantom><mfrac><mi>x</mi><mi>y</mi></mfrac></mphantom><mi>y</mi>")
            .unwrap();
    assert_eq!(hidden.tree.metrics(), visible.tree.metrics());
    assert_eq!(hidden.glyphs.len(), 2);
    assert_eq!(hidden.glyphs[0], visible.glyphs[0]);
    assert_eq!(hidden.glyphs[1], *visible.glyphs.last().unwrap());
    assert!(hidden.tree.ink_bounds().is_some());
    assert!(!svg(&hidden).contains("<rect"));
    assert!(svg(&hidden).contains("<path"));
}

#[test]
fn phantom_still_requires_its_contents_to_lay_out() {
    assert_eq!(
        render("<mphantom><mi>q</mi></mphantom>")
            .unwrap_err()
            .code(),
        "MISSING_GLYPH"
    );
    assert_eq!(
        render("<mphantom><mspace/></mphantom>").unwrap_err(),
        LayoutError::NotLaidOutYet
    );
}

#[test]
fn phantom_operators_keep_spacing_and_stretch_without_reappearing() {
    let visible = render("<mrow><mo>+</mo></mrow>").unwrap();
    let hidden = render("<mphantom><mo>+</mo></mphantom>").unwrap();
    assert_eq!(hidden.tree.metrics(), visible.tree.metrics());
    assert!(hidden.glyphs.is_empty());
    for wrapper in ["mstyle", "mrow", "mphantom"] {
        let visible = render(&format!(
            "<{wrapper}><mrow><mo>(</mo></mrow></{wrapper}><mfrac><mi>x</mi><mi>y</mi></mfrac>"
        ))
        .unwrap();
        let hidden = render(&format!("<{wrapper}><mphantom><mo>(</mo></mphantom></{wrapper}><mfrac><mi>x</mi><mi>y</mi></mfrac>")).unwrap();
        assert_eq!(hidden.tree.metrics(), visible.tree.metrics(), "{wrapper}");
        assert_eq!(hidden.glyphs.len(), 2, "{wrapper}");
        assert!(hidden.glyphs.iter().all(|glyph| glyph.character != '('));
    }
}
