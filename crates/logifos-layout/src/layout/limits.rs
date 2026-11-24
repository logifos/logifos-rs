//! Underscripts and overscripts, per MathML Core sections 3.4.2.3 to 3.4.2.5
//! with the OpenType MATH limit constants. Scripts are centred on the base.

use super::baseline::{ink_ascent, ink_descent};
use super::fraction::centered;
use super::{operator, LayoutError, Node, PlacedGlyph, ResolvedStyle};
use crate::font::math::MathConstant;
use crate::font::FontProvider;
use crate::{BoxTree, Kern, Point, StackItem};

/// Lays out a base with an underscript, an overscript or both. `accent` is
/// the overscript's `accent` attribute. Stretching along the inline axis is
/// not applied: an operator base keeps its natural width.
pub(super) fn limits<'a>(
    base: Node<'a>,
    under: Option<Node<'a>>,
    over: Option<Node<'a>>,
    accent: bool,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
) -> Result<Node<'a>, LayoutError> {
    let size = style.font_size_px;
    let largeop = base.operator.is_some_and(operator::large);
    let base_ascent = ink_ascent(&base.tree);
    let base_descent = ink_descent(&base.tree);
    let mut constant = |c: MathConstant| fonts.math_constant_px(c, size);

    // How far the overscript baseline rises above the base baseline.
    let mut rise = 0.0;
    let mut extra_ascender = 0.0;
    if let Some(over) = &over {
        let descent = ink_descent(&over.tree);
        rise = if largeop {
            constant(MathConstant::UpperLimitBaselineRiseMin)?
                .max(base_ascent + constant(MathConstant::UpperLimitGapMin)? + descent)
        } else if accent {
            base_ascent
                + (constant(MathConstant::AccentBaseHeight)? - base.tree.metrics().height())
                    .max(0.0)
        } else {
            base_ascent + constant(MathConstant::OverbarVerticalGap)? + descent
        };
        if !largeop {
            extra_ascender = constant(MathConstant::OverbarExtraAscender)?;
        }
    }
    // How far the underscript baseline drops below the base baseline.
    let mut drop = 0.0;
    let mut extra_descender = 0.0;
    if let Some(under) = &under {
        let ascent = ink_ascent(&under.tree);
        drop = if largeop {
            constant(MathConstant::LowerLimitBaselineDropMin)?
                .max(base_descent + constant(MathConstant::LowerLimitGapMin)? + ascent)
        } else {
            base_descent + constant(MathConstant::UnderbarVerticalGap)? + ascent
        };
        if !largeop {
            extra_descender = constant(MathConstant::UnderbarExtraDescender)?;
        }
    }

    let metrics = |tree: &BoxTree| tree.metrics();
    let mut line_ascent = metrics(&base.tree).height();
    let mut line_descent = metrics(&base.tree).depth();
    if let Some(over) = &over {
        line_ascent = line_ascent.max(metrics(&over.tree).height() + rise + extra_ascender);
    }
    if let Some(under) = &under {
        line_descent = line_descent.max(metrics(&under.tree).depth() + drop + extra_descender);
    }
    let mut width = metrics(&base.tree).width();
    for node in [&under, &over].into_iter().flatten() {
        width = width.max(metrics(&node.tree).width());
    }

    // Parts in top-to-bottom order, each with its top edge below the box top.
    struct Part {
        tree: BoxTree,
        glyphs: Vec<PlacedGlyph>,
        top: f64,
        key: usize,
    }
    let mut parts = Vec::new();
    let empty = base.empty
        && under.as_ref().is_none_or(|n| n.empty)
        && over.as_ref().is_none_or(|n| n.empty);
    let operator = base.operator.filter(|text| !operator::is_fence(text));
    let (operator_font_size_px, right_space) = (
        operator.and(base.operator_font_size_px),
        operator.and(base.right_space),
    );
    if let Some(over) = over {
        let top = line_ascent - rise - metrics(&over.tree).height();
        parts.push(Part {
            tree: over.tree,
            glyphs: over.glyphs,
            top,
            key: 2,
        });
    }
    let base_top = line_ascent - metrics(&base.tree).height();
    parts.push(Part {
        tree: base.tree,
        glyphs: base.glyphs,
        top: base_top,
        key: 0,
    });
    if let Some(under) = under {
        let top = line_ascent + drop - metrics(&under.tree).height();
        parts.push(Part {
            tree: under.tree,
            glyphs: under.glyphs,
            top,
            key: 1,
        });
    }

    let mut items = Vec::new();
    let mut cursor = 0.0;
    let mut widths = Vec::new();
    let mut keyed = Vec::new();
    for part in parts {
        items.push(StackItem::Kern(Kern::new(part.top - cursor)?));
        cursor = part.top + part.tree.metrics().total_height();
        widths.push(part.tree.metrics().width());
        keyed.push((part.key, part.glyphs));
        items.push(StackItem::Box(centered(part.tree, width)?));
    }
    items.push(StackItem::Kern(Kern::new(
        line_ascent + line_descent - cursor,
    )?));
    let tree = BoxTree::vbox(items, line_ascent)?;

    let mut placed = Vec::new();
    for (((key, mut glyphs), own_width), child) in
        keyed.into_iter().zip(widths).zip(tree.children())
    {
        let offset = Point::new(
            child.offset().x() + (width - own_width) / 2.0,
            child.offset().y(),
        )?;
        for glyph in &mut glyphs {
            glyph.origin = glyph.origin.translated(offset)?;
        }
        placed.push((key, glyphs));
    }
    placed.sort_by_key(|(key, _)| *key);
    Ok(Node {
        tree,
        glyphs: placed.into_iter().flat_map(|(_, glyphs)| glyphs).collect(),
        operator,
        operator_font_size_px,
        italic: 0.0,
        empty,
        right_space,
    })
}
