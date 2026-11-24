//! Scripts: a base with a raised superscript or lowered subscript, per
//! MathML Core section 3.4.1 with the OpenType MATH constants.

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
        italic: correction,
        empty: base.empty && script_empty,
    })
}

/// Wraps a script whose baseline rises by `shift` above the row baseline,
/// reporting the script's own y offset inside the wrapper.
fn raised(script: BoxTree, shift: f64) -> Result<(BoxTree, f64), crate::GeometryError> {
    let height = script.metrics().height();
    let pad = (shift - script.metrics().depth()).max(0.0);
    let items = if pad > 0.0 {
        vec![StackItem::Box(script), StackItem::Kern(Kern::new(pad)?)]
    } else {
        vec![StackItem::Box(script)]
    };
    let wrapper = BoxTree::vbox(items, height + shift)?;
    Ok((wrapper, 0.0))
}

/// Wraps a script whose baseline drops by `shift` below the row baseline,
/// reporting the script's own y offset inside the wrapper.
fn lowered(script: BoxTree, shift: f64) -> Result<(BoxTree, f64), crate::GeometryError> {
    let height = script.metrics().height();
    let pad = (shift - height).max(0.0);
    let items = if pad > 0.0 {
        vec![StackItem::Kern(Kern::new(pad)?), StackItem::Box(script)]
    } else {
        vec![StackItem::Box(script)]
    };
    let wrapper = BoxTree::vbox(items, (height - shift).max(0.0))?;
    Ok((wrapper, pad))
}

/// The ink's rise above the baseline; 0 for a box without ink.
fn ink_ascent(tree: &BoxTree) -> f64 {
    tree.ink_bounds()
        .map_or(0.0, |b| tree.metrics().baseline() - b.origin().y())
}

/// The ink's drop below the baseline; 0 for a box without ink.
fn ink_descent(tree: &BoxTree) -> f64 {
    tree.ink_bounds().map_or(0.0, |b| {
        b.origin().y() + b.height() - tree.metrics().baseline()
    })
}
