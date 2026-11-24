//! Scripts: a base with a raised superscript or lowered subscript, per
//! MathML Core section 3.4.1 with the OpenType MATH constants.

use super::baseline::{ink_ascent, ink_descent, lowered, raised};
use super::{LayoutError, Node, PlacedGlyph, ResolvedStyle};
use crate::font::math::MathConstant;
use crate::font::FontProvider;
use crate::geometry::add;
use crate::{BoxTree, Kern, Point, StackItem};

pub(super) fn superscript<'a>(
    base: Node<'a>,
    script: Node<'a>,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
) -> Result<Node<'a>, LayoutError> {
    let Node {
        tree: script_tree,
        glyphs: script_glyphs,
        empty: script_empty,
        ..
    } = script;
    // The correction is the base's italic correction unless the base is a
    // largeop embellishment (whose correction is 0 here, as in the spec's
    // LargeOpItalicCorrection branch).
    let correction = base.italic;
    let shift = {
        let constant = if style.cramped {
            MathConstant::SuperscriptShiftUpCramped
        } else {
            MathConstant::SuperscriptShiftUp
        };
        fonts
            .math_constant_px(constant, style.font_size_px)?
            .max(
                fonts.math_constant_px(MathConstant::SuperscriptBottomMin, style.font_size_px)?
                    + ink_descent(&script_tree),
            )
            .max(
                ink_ascent(&base.tree)
                    - fonts.math_constant_px(
                        MathConstant::SuperscriptBaselineDropMax,
                        style.font_size_px,
                    )?,
            )
    };
    let placed = raised(script_tree, shift)?;
    scripted(
        base,
        script_glyphs,
        script_empty,
        correction,
        placed,
        fonts,
        style,
    )
}

pub(super) fn subscript<'a>(
    base: Node<'a>,
    script: Node<'a>,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
) -> Result<Node<'a>, LayoutError> {
    let Node {
        tree: script_tree,
        glyphs: script_glyphs,
        empty: script_empty,
        ..
    } = script;
    // A largeop base's correction is what shows outside the construct; no
    // largeop embellishment exists yet, so nothing is carried here.
    let correction = 0.0;
    let shift = fonts
        .math_constant_px(MathConstant::SubscriptShiftDown, style.font_size_px)?
        .max(
            ink_ascent(&script_tree)
                - fonts.math_constant_px(MathConstant::SubscriptTopMax, style.font_size_px)?,
        )
        .max(
            fonts.math_constant_px(MathConstant::SubscriptBaselineDropMin, style.font_size_px)?
                + ink_descent(&base.tree),
        );
    let placed = lowered(script_tree, shift)?;
    scripted(
        base,
        script_glyphs,
        script_empty,
        correction,
        placed,
        fonts,
        style,
    )
}

/// Places the base, the correction kern, the re-baselined script and the
/// trailing script space on one baseline row.
#[allow(clippy::too_many_arguments)]
fn scripted<'a>(
    base: Node<'a>,
    script_glyphs: Vec<PlacedGlyph>,
    script_empty: bool,
    correction: f64,
    (script_tree, script_inner_y): (BoxTree, f64),
    fonts: &mut FontProvider,
    style: ResolvedStyle,
) -> Result<Node<'a>, LayoutError> {
    let space = fonts.math_constant_px(MathConstant::SpaceAfterScript, style.font_size_px)?;
    let tree = BoxTree::hbox(vec![
        StackItem::Box(base.tree),
        StackItem::Kern(Kern::new(correction)?),
        StackItem::Box(script_tree),
        StackItem::Kern(Kern::new(space)?),
    ])?;
    let base_offset = tree.children()[0].offset();
    let script_offset = tree.children()[1].offset();
    let mut glyphs = base.glyphs;
    for glyph in &mut glyphs {
        glyph.origin = glyph.origin.translated(base_offset)?;
    }
    let script_shift = Point::new(script_offset.x(), add(script_offset.y(), script_inner_y)?)?;
    for mut glyph in script_glyphs {
        glyph.origin = glyph.origin.translated(script_shift)?;
        glyphs.push(glyph);
    }
    Ok(Node {
        tree,
        glyphs,
        operator: base.operator,
        operator_font_size_px: base.operator_font_size_px,
        italic: correction,
        empty: base.empty && script_empty,
    })
}
