//! Fractions: a centered numerator and denominator around an axis-aligned
//! rule, per MathML Core section 4.2 with the OpenType MATH constants.
//! The bevelled spelling (MathML 3) lays the children on a diagonal with
//! the font's solidus as the divider.

use super::baseline::{lowered, raised};
use super::{LayoutError, Node, PlacedGlyph, ResolvedStyle};
use crate::font::math::MathConstant;
use crate::font::FontProvider;
use crate::geometry::add;
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

/// The bevelled fraction: the numerator sits raised and the denominator
/// lowered on the row baseline, with the font's solidus between them and
/// no operator spacing (the construct owns the arrangement). MathML Core
/// dropped the attribute, so the shifts are the inline fraction constants.
pub(super) fn bevelled<'a>(
    numerator: Node<'a>,
    denominator: Node<'a>,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
) -> Result<Node<'a>, LayoutError> {
    // The divider matches the compact children's size, not the row's.
    let child_size = style
        .raise(fonts, u8::from(!style.display), style.cramped)?
        .font_size_px;
    let up = fonts.math_constant_px(MathConstant::FractionNumeratorShiftUp, child_size)?;
    let down = fonts.math_constant_px(MathConstant::FractionDenominatorShiftDown, child_size)?;
    let measured = fonts.measure('/', child_size)?;
    eprintln!("DBG2 child_size={child_size} up={up} down={down}");
    let divider_metrics = measured.metrics;
    let divider = Node {
        tree: BoxTree::leaf(divider_metrics, measured.ink_bounds),
        glyphs: vec![PlacedGlyph {
            character: '/',
            glyph: measured.glyph,
            origin: Point::new(0.0, divider_metrics.baseline())?,
            font_size_px: child_size,
            color: style.color,
        }],
        operator: None,
        operator_font_size_px: None,
        italic: 0.0,
        empty: false,
        right_space: None,
        max_size_percent: None,
    };
    let (numerator_tree, numerator_y) = raised(numerator.tree, up)?;
    let (denominator_tree, denominator_y) = lowered(denominator.tree, down)?;
    let tree = BoxTree::hbox(vec![
        StackItem::Box(numerator_tree),
        StackItem::Box(divider.tree),
        StackItem::Box(denominator_tree),
    ])?;
    let numerator_offset = tree.children()[0].offset();
    let divider_offset = tree.children()[1].offset();
    let denominator_offset = tree.children()[2].offset();
    let mut glyphs = numerator.glyphs;
    let shift = Point::new(
        numerator_offset.x(),
        add(numerator_offset.y(), numerator_y)?,
    )?;
    for glyph in &mut glyphs {
        glyph.origin = glyph.origin.translated(shift)?;
    }
    let mut glyphs = {
        let mut divider_glyphs = divider.glyphs;
        for glyph in &mut divider_glyphs {
            glyph.origin = glyph.origin.translated(divider_offset)?;
        }
        glyphs.extend(divider_glyphs);
        glyphs
    };
    let shift = Point::new(
        denominator_offset.x(),
        add(denominator_offset.y(), denominator_y)?,
    )?;
    for mut glyph in denominator.glyphs {
        glyph.origin = glyph.origin.translated(shift)?;
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
