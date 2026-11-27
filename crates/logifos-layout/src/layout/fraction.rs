//! Fractions: a centered numerator and denominator around an axis-aligned
//! rule, per MathML Core section 4.2 with the OpenType MATH constants.

use super::{LayoutError, Node, ResolvedStyle};
use crate::font::math::MathConstant;
use crate::font::FontProvider;
use crate::{BoxTree, Kern, Point, StackItem};

/// The rule thickness, shifts and minimum gaps for this call, in CSS pixels.
struct Metrics {
    rule: f64,
    numerator_shift: f64,
    denominator_shift: f64,
    numerator_gap: f64,
    denominator_gap: f64,
}

pub(super) fn fraction<'a>(
    numerator: Node<'a>,
    denominator: Node<'a>,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
) -> Result<Node<'a>, LayoutError> {
    let metrics = Metrics::resolve(fonts, style)?;
    let numerator_tree = numerator.tree;
    let denominator_tree = denominator.tree;
    let (numerator_width, numerator_height, numerator_depth) = (
        numerator_tree.metrics().width(),
        numerator_tree.metrics().height(),
        numerator_tree.metrics().depth(),
    );
    let (denominator_width, denominator_height) = (
        denominator_tree.metrics().width(),
        denominator_tree.metrics().height(),
    );
    let width = numerator_width.max(denominator_width);
    // The rule keeps at least the minimum gap to each child's ink; the shift
    // grows when the child's far side would come closer than that.
    let shift_up = metrics
        .numerator_shift
        .max(numerator_depth + metrics.rule / 2.0 + metrics.numerator_gap);
    let shift_down = metrics
        .denominator_shift
        .max(denominator_height + metrics.rule / 2.0 + metrics.denominator_gap);

    let numerator_gap_kern = shift_up - numerator_depth - metrics.rule / 2.0;
    let denominator_gap_kern = shift_down - denominator_height - metrics.rule / 2.0;
    let numerator_top = centered(numerator_tree, width)?;
    let denominator_top = centered(denominator_tree, width)?;
    let denominator_offset = Point::new(
        (width - denominator_width) / 2.0,
        numerator_height
            + numerator_depth
            + numerator_gap_kern
            + metrics.rule
            + denominator_gap_kern,
    )?;
    let baseline = numerator_height + shift_up;
    let tree = BoxTree::vbox(
        vec![
            StackItem::Box(numerator_top),
            StackItem::Kern(Kern::new(numerator_gap_kern)?),
            StackItem::Box(BoxTree::rule(width, metrics.rule, 0.0)?),
            StackItem::Kern(Kern::new(denominator_gap_kern)?),
            StackItem::Box(denominator_top),
        ],
        baseline,
    )?;

    let mut glyphs = numerator.glyphs;
    let offset = Point::new((width - numerator_width) / 2.0, 0.0)?;
    for glyph in &mut glyphs {
        glyph.origin = glyph.origin.translated(offset)?;
    }
    for mut glyph in denominator.glyphs {
        glyph.origin = glyph.origin.translated(denominator_offset)?;
        glyphs.push(glyph);
    }
    Ok(Node {
        tree,
        glyphs,
        operator: None,
        operator_font_size_px: None,
        italic: 0.0,
        empty: false,
        right_space: None,
        max_size_percent: None,
    })
}

impl Metrics {
    /// Display style applies at script level zero; the display variants of
    /// the shifts and gaps are the block-form values.
    fn resolve(fonts: &mut FontProvider, style: ResolvedStyle) -> Result<Self, LayoutError> {
        let size = style.font_size_px;
        let display = style.display;
        let (numerator_shift, denominator_shift, numerator_gap, denominator_gap) = if display {
            (
                MathConstant::FractionNumeratorDisplayStyleShiftUp,
                MathConstant::FractionDenominatorDisplayStyleShiftDown,
                MathConstant::FractionNumDisplayStyleGapMin,
                MathConstant::FractionDenomDisplayStyleGapMin,
            )
        } else {
            (
                MathConstant::FractionNumeratorShiftUp,
                MathConstant::FractionDenominatorShiftDown,
                MathConstant::FractionNumeratorGapMin,
                MathConstant::FractionDenominatorGapMin,
            )
        };
        let rule = fonts.math_constant_px(MathConstant::FractionRuleThickness, size)?;
        Ok(Self {
            rule,
            numerator_shift: fonts.math_constant_px(numerator_shift, size)?,
            denominator_shift: fonts.math_constant_px(denominator_shift, size)?,
            numerator_gap: fonts.math_constant_px(numerator_gap, size)?,
            denominator_gap: fonts.math_constant_px(denominator_gap, size)?,
        })
    }
}

/// Wraps the child so it is horizontally centered in `width`: its glyphs and
/// its own width are unchanged, its left edge moves to the pad.
pub(super) fn centered(tree: BoxTree, width: f64) -> Result<BoxTree, crate::GeometryError> {
    let pad = (width - tree.metrics().width()) / 2.0;
    BoxTree::hbox(vec![
        StackItem::Kern(Kern::new(pad)?),
        StackItem::Box(tree),
        StackItem::Kern(Kern::new(pad)?),
    ])
}
