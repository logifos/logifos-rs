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
    let shift = super_shift(&base.tree, &script_tree, fonts, style)?;
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
    let shift = sub_shift(&base.tree, &script_tree, fonts, style)?;
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
        right_space: base.right_space,
        max_size_percent: None,
    })
}

fn super_shift(
    base: &BoxTree,
    script: &BoxTree,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
) -> Result<f64, LayoutError> {
    let constant = if style.cramped {
        MathConstant::SuperscriptShiftUpCramped
    } else {
        MathConstant::SuperscriptShiftUp
    };
    let size = style.font_size_px;
    Ok(fonts
        .math_constant_px(constant, size)?
        .max(
            fonts.math_constant_px(MathConstant::SuperscriptBottomMin, size)? + ink_descent(script),
        )
        .max(
            ink_ascent(base)
                - fonts.math_constant_px(MathConstant::SuperscriptBaselineDropMax, size)?,
        ))
}

fn sub_shift(
    base: &BoxTree,
    script: &BoxTree,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
) -> Result<f64, LayoutError> {
    let size = style.font_size_px;
    Ok(fonts
        .math_constant_px(MathConstant::SubscriptShiftDown, size)?
        .max(ink_ascent(script) - fonts.math_constant_px(MathConstant::SubscriptTopMax, size)?)
        .max(
            fonts.math_constant_px(MathConstant::SubscriptBaselineDropMin, size)?
                + ink_descent(base),
        ))
}

/// One subscript/superscript pair; a missing script is an empty box.
pub(super) struct Pair<'a> {
    pub sub: Option<Node<'a>>,
    pub sup: Option<Node<'a>>,
}

struct Prepared {
    tree: BoxTree,
    glyphs: Vec<PlacedGlyph>,
    empty: bool,
}

impl Prepared {
    fn new(node: Option<Node<'_>>) -> Result<Self, LayoutError> {
        Ok(match node {
            Some(node) => Self {
                tree: node.tree,
                glyphs: node.glyphs,
                empty: node.empty,
            },
            None => Self {
                tree: BoxTree::space(crate::Metrics::new(0.0, 0.0, 0.0)?),
                glyphs: Vec::new(),
                empty: true,
            },
        })
    }
}

/// Core 3.4.1.4 and 3.4.3.1: msubsup and mmultiscripts. The shared shifts are
/// the maximum over the pairs, each adjusted for the sub/super gap.
pub(super) fn multi<'a>(
    base: Node<'a>,
    post: Vec<Pair<'a>>,
    pre: Vec<Pair<'a>>,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
) -> Result<Node<'a>, LayoutError> {
    let size = style.font_size_px;
    let space = fonts.math_constant_px(MathConstant::SpaceAfterScript, size)?;
    let gap_min = fonts.math_constant_px(MathConstant::SubSuperscriptGapMin, size)?;
    let max_with_sub =
        fonts.math_constant_px(MathConstant::SuperscriptBottomMaxWithSubscript, size)?;
    let prepare = |pairs: Vec<Pair<'a>>| -> Result<Vec<(Prepared, Prepared)>, LayoutError> {
        pairs
            .into_iter()
            .map(|p| Ok((Prepared::new(p.sub)?, Prepared::new(p.sup)?)))
            .collect()
    };
    let post = prepare(post)?;
    let pre = prepare(pre)?;
    let mut sub_max = 0.0f64;
    let mut sup_max = 0.0f64;
    for (sub, sup) in post.iter().chain(pre.iter()) {
        let mut up = super_shift(&base.tree, &sup.tree, fonts, style)?;
        let mut down = sub_shift(&base.tree, &sub.tree, fonts, style)?;
        let gap = (down - ink_ascent(&sub.tree)) + (up - ink_descent(&sup.tree));
        if gap < gap_min {
            let delta = max_with_sub - (up - ink_descent(&sup.tree));
            if delta > 0.0 {
                up += delta.min(gap_min - gap);
            }
            let gap = (down - ink_ascent(&sub.tree)) + (up - ink_descent(&sup.tree));
            let delta = gap_min - gap;
            if delta > 0.0 {
                down += delta;
            }
        }
        sub_max = sub_max.max(down);
        sup_max = sup_max.max(up);
    }
    let correction = base.italic;
    let mut all_empty = base.empty;
    // Glyphs and inner y per box item, in hbox order, with the source-order key.
    let mut items = Vec::new();
    let mut pieces: Vec<(Vec<PlacedGlyph>, f64, usize)> = Vec::new();
    // Source order: base, postscripts, prescripts.
    let post_keys: Vec<_> = (0..post.len()).map(|i| (1 + 2 * i, 2 + 2 * i)).collect();
    let pre_start = 1 + 2 * post.len();
    let pre_keys: Vec<_> = (0..pre.len())
        .map(|i| (pre_start + 2 * i, pre_start + 1 + 2 * i))
        .collect();
    for ((sub, sup), (sub_key, sup_key)) in pre.into_iter().zip(pre_keys) {
        all_empty &= sub.empty && sup.empty;
        let (sub_width, sup_width) = (sub.tree.metrics().width(), sup.tree.metrics().width());
        let pair = sub_width.max(sup_width);
        let (sub_tree, sub_y) = lowered(sub.tree, sub_max)?;
        let (sup_tree, sup_y) = raised(sup.tree, sup_max)?;
        items.push(StackItem::Kern(Kern::new(space + pair - sub_width)?));
        items.push(StackItem::Box(sub_tree));
        pieces.push((sub.glyphs, sub_y, sub_key));
        items.push(StackItem::Kern(Kern::new(-sup_width)?));
        items.push(StackItem::Box(sup_tree));
        pieces.push((sup.glyphs, sup_y, sup_key));
    }
    items.push(StackItem::Box(base.tree));
    pieces.push((base.glyphs, 0.0, 0));
    for ((sub, sup), (sub_key, sup_key)) in post.into_iter().zip(post_keys) {
        all_empty &= sub.empty && sup.empty;
        let (sub_width, sup_width) = (sub.tree.metrics().width(), sup.tree.metrics().width());
        let pair = sub_width.max(sup_width);
        let (sub_tree, sub_y) = lowered(sub.tree, sub_max)?;
        let (sup_tree, sup_y) = raised(sup.tree, sup_max)?;
        // Cursor moves relative to the pair start: subscript at 0, the
        // superscript at the italic correction, then the pair width and space.
        items.push(StackItem::Box(sub_tree));
        pieces.push((sub.glyphs, sub_y, sub_key));
        items.push(StackItem::Kern(Kern::new(correction - sub_width)?));
        items.push(StackItem::Box(sup_tree));
        pieces.push((sup.glyphs, sup_y, sup_key));
        items.push(StackItem::Kern(Kern::new(
            pair + space - correction - sup_width,
        )?));
    }
    let tree = BoxTree::hbox(items)?;
    let mut placed: Vec<(usize, Vec<PlacedGlyph>)> = Vec::new();
    for ((mut glyphs, inner_y, key), child) in pieces.into_iter().zip(tree.children()) {
        let offset = Point::new(child.offset().x(), add(child.offset().y(), inner_y)?)?;
        for glyph in &mut glyphs {
            glyph.origin = glyph.origin.translated(offset)?;
        }
        placed.push((key, glyphs));
    }
    placed.sort_by_key(|(key, _)| *key);
    let operator = base
        .operator
        .filter(|text| !super::operator::is_fence(text));
    Ok(Node {
        tree,
        glyphs: placed.into_iter().flat_map(|(_, g)| g).collect(),
        operator,
        operator_font_size_px: operator.and(base.operator_font_size_px),
        italic: 0.0,
        empty: all_empty,
        right_space: operator.and(base.right_space),
        max_size_percent: None,
    })
}
