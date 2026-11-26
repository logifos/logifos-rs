//! MathML-3 enclosures: the content draws inside rule-drawn borders. The
//! box family and the bottom rule draw as filled rules; the curved and
//! diagonal notations land with the arc primitives.

use super::{build, LayoutError, Node, ResolvedStyle};
use crate::font::math::MathConstant;
use crate::font::FontProvider;
use crate::{BoxKind, BoxTree, Metrics, Point, PositionedBox};
use logifos_model::Formula;

pub(super) fn enclose<'a>(
    notation: &str,
    content: &'a Formula,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
    budget: &mut Budget,
    depth: usize,
) -> Result<Node<'a>, LayoutError> {
    let names: Vec<&str> = notation.split_whitespace().collect();
    // The box family and the bottom rule draw as rules; the curved and
    // diagonal spellings land with the arc primitives and refuse typed
    // until then.
    let bottom_only = matches!(names.as_slice(), ["bottom"]);
    let full_box = names.is_empty() || matches!(names.as_slice(), ["box"]);
    if !bottom_only && !full_box {
        return Err(LayoutError::NotLaidOutYet);
    }
    let thickness =
        fonts.math_constant_px(MathConstant::FractionRuleThickness, style.font_size_px)?;
    let pad = thickness;
    let inner = build(content, fonts, style, budget, depth + 1)?;
    let inner_w = inner.tree.metrics().width();
    let inner_h = inner.tree.metrics().height();
    let inner_d = inner.tree.metrics().depth();
    let width = inner_w + pad * 2.0;
    let height = inner_h + pad;
    let depth_out = inner_d + pad;
    let metrics = Metrics::new(width, height, depth_out)?;

    let mut children = Vec::new();
    children.push(PositionedBox::place(Point::new(pad, pad)?, inner.tree));
    if bottom_only {
        let y = pad + inner_h + inner_d;
        children.push(PositionedBox::place(
            Point::new(pad, y)?,
            BoxTree::rule(inner_w, thickness, 0.0)?,
        ));
    } else {
        let total = height + depth_out;
        children.push(PositionedBox::place(
            Point::new(0.0, 0.0)?,
            BoxTree::rule(width, thickness, 0.0)?,
        ));
        children.push(PositionedBox::place(
            Point::new(0.0, total - thickness)?,
            BoxTree::rule(width, thickness, 0.0)?,
        ));
        children.push(PositionedBox::place(
            Point::new(0.0, thickness)?,
            BoxTree::rule(thickness, total - thickness * 2.0, 0.0)?,
        ));
        children.push(PositionedBox::place(
            Point::new(width - thickness, thickness)?,
            BoxTree::rule(thickness, total - thickness * 2.0, 0.0)?,
        ));
    }
    let tree = BoxTree::composed(BoxKind::HBox, metrics, children)?;

    let mut glyphs = inner.glyphs;
    for glyph in &mut glyphs {
        glyph.origin = glyph.origin.translated(Point::new(pad, pad)?)?;
    }
    Ok(Node {
        tree,
        glyphs,
        operator: None,
        operator_font_size_px: None,
        italic: 0.0,
        empty: false,
        right_space: None,
    })
}

use super::Budget;
