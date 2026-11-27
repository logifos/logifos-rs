use super::{operator, Budget, LayoutError, Node, PlacedGlyph, ResolvedStyle};
use crate::font::math::{Direction, MathConstant};
use crate::font::FontProvider;
use crate::{BoxTree, Metrics, Point};

/// The invisible operators (Core 3.2.4): zero advance, zero spacing, no ink.
const INVISIBLE_OPERATORS: [&str; 4] = ["\u{2061}", "\u{2062}", "\u{2063}", "\u{2064}"];

pub(super) fn token<'a>(
    text: &str,
    operator: Option<&'a str>,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
    budget: &mut Budget,
) -> Result<Node<'a>, LayoutError> {
    if operator.is_some_and(|text| text.is_empty()) {
        // An empty operator has no ink, spacing or embellishment.
        return super::rows::stack(Vec::new());
    }
    if operator.is_some_and(|text| INVISIBLE_OPERATORS.contains(&text)) {
        // Core 3.2.4: the four invisible operators carry no glyph and no
        // advance; spacing treats them as absent entirely.
        return super::rows::stack(Vec::new());
    }
    if style.display && operator.is_some_and(operator::large) {
        return display_operator(text, operator, fonts, style);
    }
    let mut children = Vec::new();
    let mut italic = 0.0;
    for character in text.chars() {
        let character = match style.variant {
            Some(variant) if operator.is_none() => super::variant::map(character, variant),
            _ => character,
        };
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
                    color: style.color,
                }],
                operator: None,
                operator_font_size_px: None,
                italic: 0.0,
                empty: false,
                right_space: None,
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

/// Core 3.2.4.2 largeop in display style: the first vertical variant at least
/// DisplayOperatorMinHeight tall (or the assembly), centred on the math axis.
fn display_operator<'a>(
    text: &str,
    operator: Option<&'a str>,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
) -> Result<Node<'a>, LayoutError> {
    let mut chars = text.chars();
    let (Some(character), None) = (chars.next(), chars.next()) else {
        return Err(LayoutError::UnsupportedOperator(text.to_owned()));
    };
    let primary = fonts.primary();
    let units = primary.math.constants.display_operator_min_height;
    let minimum = primary.scale(f64::from(units), style.font_size_px)?;
    let axis = fonts.math_constant_px(MathConstant::AxisHeight, style.font_size_px)?;
    let stretched =
        fonts.expand_stretch(character, Direction::Vertical, minimum, style.font_size_px)?;
    let extent = stretched.extent_px;
    let ascent = (axis + extent / 2.0).max(0.0);
    let descent = (extent / 2.0 - axis).max(0.0);
    let tree = BoxTree::leaf(Metrics::new(stretched.advance_px, ascent, descent)?, None);
    let glyphs = super::radical::stretched_pieces(
        &stretched,
        character,
        ascent + descent - extent,
        style.font_size_px,
        style.color,
    )?;
    Ok(Node {
        tree,
        glyphs,
        operator,
        operator_font_size_px: Some(style.font_size_px),
        italic: 0.0,
        empty: false,
        right_space: None,
    })
}
