use super::{Budget, LayoutError, Node, PlacedGlyph, ResolvedStyle};
use crate::font::FontProvider;
use crate::{BoxTree, Point};

pub(super) fn token<'a>(
    text: &str,
    operator: Option<&'a str>,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
    budget: &mut Budget,
) -> Result<Node<'a>, LayoutError> {
    let mut children = Vec::new();
    let mut italic = 0.0;
    for character in text.chars() {
        budget.glyph()?;
        let measured = fonts.measure(character, style.font_size_px)?;
        italic = measured.glyph.italics_correction_px;
        let metrics = measured.metrics;
        children.push((
            Node {
                tree: BoxTree::leaf(metrics, measured.ink_bounds),
                glyphs: vec![PlacedGlyph {
                    character,
                    glyph: measured.glyph,
                    origin: Point::new(0.0, metrics.baseline())?,
                    font_size_px: style.font_size_px,
                }],
                operator: None,
                operator_font_size_px: None,
                italic: 0.0,
                empty: false,
            },
            0.0,
            0.0,
        ));
    }
    let mut node = super::rows::stack(children)?;
    node.operator = operator;
    node.operator_font_size_px = operator.map(|_| style.font_size_px);
    node.italic = if operator.is_some() { 0.0 } else { italic };
    Ok(node)
}
