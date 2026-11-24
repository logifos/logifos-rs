//! Radicals: a stretched surd, an overbar rule and the radicand, per
//! MathML Core section 3.3.3 with the OpenType MATH constants.

use super::baseline::{ink_ascent, ink_descent, raised};
use super::{LayoutError, Node, PlacedGlyph, ResolvedStyle};
use crate::font::math::{Direction, MathConstant};
use crate::font::{FontProvider, StretchedGlyph};
use crate::geometry::{add, subtract};
use crate::{BoxTree, Kern, Metrics, Point, StackItem};

/// The share of a percent constant applied per unit.
const PERCENT: f64 = 0.01;

pub(super) fn sqrt<'a>(
    radicand: Node<'a>,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
) -> Result<Node<'a>, LayoutError> {
    surd_row(radicand, fonts, style).map(|(row, _, _)| row)
}

pub(super) fn root<'a>(
    radicand: Node<'a>,
    index: Node<'a>,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
) -> Result<Node<'a>, LayoutError> {
    let (b, _, _) = surd_row(radicand, fonts, style)?;
    let Node {
        tree: index_tree,
        glyphs: index_glyphs,
        ..
    } = index;
    let (b_height, b_depth) = (b.tree.metrics().height(), b.tree.metrics().depth());
    let (index_width, index_height) = (index_tree.metrics().width(), index_tree.metrics().height());
    let percent = f64::from(
        fonts
            .primary()
            .math
            .constants
            .radical_degree_bottom_raise_percent,
    ) * PERCENT;
    let kern_before =
        fonts.math_constant_px(MathConstant::RadicalKernBeforeDegree, style.font_size_px)?;
    let kern_after =
        fonts.math_constant_px(MathConstant::RadicalKernAfterDegree, style.font_size_px)?;
    let adjusted_before = kern_before.max(0.0);
    let adjusted_after = kern_after.max(-index_width);
    // The index top contributes -B.depth + percent * B.block + its own
    // height above the baseline; its baseline sits one height below that.
    let index_top = add(add(-b_depth, percent * (b_height + b_depth))?, index_height)?;
    let shift = subtract(index_top, index_height)?;
    let (index_wrapper, index_inner) = raised(index_tree, shift)?;
    let row = BoxTree::hbox(vec![
        StackItem::Kern(Kern::new(adjusted_before)?),
        StackItem::Box(index_wrapper),
        StackItem::Kern(Kern::new(adjusted_after)?),
        StackItem::Box(b.tree),
    ])?;
    // The spec's descent formula can exceed the children's geometric extent;
    // a trailing kern carries the difference without moving any ink.
    let geometric_depth = row.metrics().depth();
    let baseline = row.metrics().baseline();
    let descent = (b_depth - percent * (b_height + b_depth) + index_height)
        .max(b_depth)
        .max(geometric_depth);
    let (tree, wrapped) = if descent > geometric_depth {
        (
            BoxTree::vbox(
                vec![
                    StackItem::Box(row),
                    StackItem::Kern(Kern::new(descent - geometric_depth)?),
                ],
                baseline,
            )?,
            true,
        )
    } else {
        (row, false)
    };
    let row = if wrapped {
        tree.children()[0].node()
    } else {
        &tree
    };

    let index_offset = row.children()[0].offset();
    let mut glyphs = index_glyphs;
    for glyph in &mut glyphs {
        glyph.origin = glyph.origin.translated(Point::new(
            index_offset.x(),
            add(index_offset.y(), index_inner)?,
        )?)?;
    }
    let b_offset = row.children()[1].offset();
    for mut glyph in b.glyphs {
        glyph.origin = glyph.origin.translated(b_offset)?;
        glyphs.push(glyph);
    }
    Ok(Node {
        tree,
        glyphs,
        operator: None,
        italic: 0.0,
        empty: false,
    })
}

/// Lays out the surd, its overbar rule and the radicand on one baseline row,
/// returning the row with the rule thickness and extra ascender used.
fn surd_row<'a>(
    radicand: Node<'a>,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
) -> Result<(Node<'a>, f64, f64), LayoutError> {
    let gap = fonts.math_constant_px(
        if style.display {
            MathConstant::RadicalDisplayStyleVerticalGap
        } else {
            MathConstant::RadicalVerticalGap
        },
        style.font_size_px,
    )?;
    let rule = fonts.math_constant_px(MathConstant::RadicalRuleThickness, style.font_size_px)?;
    let extra = fonts.math_constant_px(MathConstant::RadicalExtraAscender, style.font_size_px)?;
    let ink_height = add(ink_ascent(&radicand.tree), ink_descent(&radicand.tree))?;
    let target = add(add(rule, gap)?, ink_height)?;
    let surd = fonts.expand_stretch('√', Direction::Vertical, target, style.font_size_px)?;
    let ascent = radicand.tree.metrics().height().max(add(
        add(ink_ascent(&radicand.tree), gap)?,
        add(rule, extra)?,
    )?);
    // The surd leaf spans the whole content ascent; its pieces hang below the
    // extra ascender, and a too-short surd keeps a nonnegative leaf depth.
    let leaf_depth = subtract(add(surd.extent_px, extra)?, ascent)?.max(0.0);
    let leaf = BoxTree::leaf(Metrics::new(surd.advance_px, ascent, leaf_depth)?, None);
    // The overbar spans the radicand with zero advance: its wrapper carries
    // the rule, and a negative kern hands the borrowed width straight back.
    let radicand_width = radicand.tree.metrics().width();
    let bar_baseline = subtract(ascent, extra)?;
    let overbar = BoxTree::rule(radicand_width, rule, 0.0)?;
    let pad = subtract(bar_baseline, rule)?.max(0.0);
    let wrapper = BoxTree::vbox(
        vec![StackItem::Box(overbar), StackItem::Kern(Kern::new(pad)?)],
        bar_baseline,
    )?;
    let tree = BoxTree::hbox(vec![
        StackItem::Box(leaf),
        StackItem::Box(wrapper),
        StackItem::Kern(Kern::new(-radicand_width)?),
        StackItem::Box(radicand.tree),
    ])?;

    let mut glyphs = surd_pieces(&surd, extra, style.font_size_px)?;
    let leaf_offset = tree.children()[0].offset();
    for glyph in &mut glyphs {
        glyph.origin = glyph.origin.translated(leaf_offset)?;
    }
    let radicand_offset = tree.children()[2].offset();
    for mut glyph in radicand.glyphs {
        glyph.origin = glyph.origin.translated(radicand_offset)?;
        glyphs.push(glyph);
    }
    Ok((
        Node {
            tree,
            glyphs,
            operator: None,
            italic: 0.0,
            empty: false,
        },
        rule,
        extra,
    ))
}

/// Places the surd pieces in the leaf's coordinates: piece baselines sit at
/// their slot bottoms below the extra ascender.
fn surd_pieces(
    surd: &StretchedGlyph,
    extra: f64,
    font_size_px: f64,
) -> Result<Vec<PlacedGlyph>, LayoutError> {
    let mut glyphs = Vec::with_capacity(surd.pieces.len());
    let mut slot = 0.0;
    for (piece, extent) in surd.pieces.iter().zip(&surd.extents) {
        let y = add(add(extra, slot)?, *extent)?;
        glyphs.push(PlacedGlyph {
            character: '√',
            glyph: piece.glyph.clone(),
            origin: Point::new(0.0, y)?,
            font_size_px,
        });
        slot = add(add(slot, *extent)?, -surd.overlap_px)?;
    }
    Ok(glyphs)
}
