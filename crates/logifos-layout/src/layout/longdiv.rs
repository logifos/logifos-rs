//! Long division: the head children compose per `longdivstyle` above the
//! working stack. The dividend's baseline is the construct's baseline; the
//! bracket draws as filled rules around it. The two corpus styles compose
//! deterministically; other spellings refuse typed at the transform.

use super::{build, Budget, LayoutError, Node, ResolvedStyle};
use crate::font::math::MathConstant;
use crate::font::FontProvider;
use crate::geometry::{add, subtract};
use crate::{BoxTree, Kern, Metrics, Point, StackItem};
use logifos_model::{LongDivStyle, LongDivision};

pub(super) fn long_division<'a>(
    division: &'a LongDivision,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
    budget: &mut Budget,
    depth: usize,
) -> Result<Node<'a>, LayoutError> {
    let mut children = division.children.iter();
    let Some(divisor) = children.next() else {
        return Ok(empty());
    };
    let Some(dividend) = children.next() else {
        return Ok(empty());
    };
    let Some(result) = children.next() else {
        return Ok(empty());
    };
    let divisor = build(divisor, fonts, style, budget, depth + 1)?;
    let dividend = build(dividend, fonts, style, budget, depth + 1)?;
    let result = build(result, fonts, style, budget, depth + 1)?;
    let working = match children.next() {
        Some(stack) => Some(build(stack, fonts, style, budget, depth + 1)?),
        None => None,
    };

    let thickness =
        fonts.math_constant_px(MathConstant::FractionRuleThickness, style.font_size_px)?;
    let pad = thickness;
    let dividend_width = dividend.tree.metrics().width();

    // The dividend block carries the horizontal rule over it, so the
    // dividend's own baseline shifts down by the rule band.
    let dividend_block = BoxTree::vbox(
        vec![
            StackItem::Kern(Kern::new(thickness)?),
            StackItem::Box(BoxTree::rule(
                add(dividend_width, pad * 2.0)?,
                thickness,
                0.0,
            )?),
            StackItem::Kern(Kern::new(thickness)?),
            StackItem::Box(dividend.tree.clone()),
        ],
        add(
            add(thickness * 2.0, thickness)?,
            dividend.tree.metrics().baseline(),
        )?,
    )?;

    let bracket_height = add(
        dividend_block.metrics().total_height(),
        working
            .as_ref()
            .map_or(0.0, |stack| stack.tree.metrics().total_height()),
    )?;
    let bracket = BoxTree::rule(thickness, bracket_height, 0.0)?;

    // The divisor block: lefttop puts it beside the dividend; the stacked
    // style puts it above the bracket line.
    let head = match division.style {
        LongDivStyle::Lefttop => BoxTree::hbox(vec![
            StackItem::Box(divisor.tree.clone()),
            StackItem::Kern(Kern::new(pad)?),
            StackItem::Box(bracket),
            StackItem::Kern(Kern::new(pad)?),
            StackItem::Box(dividend_block),
        ])?,
        LongDivStyle::Stackedleftlinetop => BoxTree::hbox(vec![
            StackItem::Box(BoxTree::vbox(
                vec![
                    StackItem::Box(divisor.tree.clone()),
                    StackItem::Kern(Kern::new(pad)?),
                    StackItem::Box(BoxTree::rule(
                        divisor.tree.metrics().width(),
                        thickness,
                        0.0,
                    )?),
                ],
                divisor.tree.metrics().baseline(),
            )?),
            StackItem::Kern(Kern::new(pad)?),
            StackItem::Box(bracket),
            StackItem::Kern(Kern::new(pad)?),
            StackItem::Box(dividend_block),
        ])?,
    };

    // The result row sits above, right-aligned to the dividend zone.
    let head_width = head.metrics().width();
    let head_total = head.metrics().total_height();
    let result_height = result.tree.metrics().height();
    let result_x = subtract(head_width, result.tree.metrics().width())?;
    let top = BoxTree::hbox(vec![
        StackItem::Kern(Kern::new(result_x.max(0.0))?),
        StackItem::Box(result.tree.clone()),
    ])?;

    let result_height_px = result.tree.metrics().height();
    let head_height = head.metrics().height();
    let head_depth = head.metrics().depth();
    let working_height = working
        .as_ref()
        .map_or(0.0, |stack| stack.tree.metrics().height());
    let working_depth = working
        .as_ref()
        .map_or(0.0, |stack| stack.tree.metrics().depth());
    let mut items = vec![
        StackItem::Kern(Kern::new(result_height)?),
        StackItem::Box(top),
        StackItem::Kern(Kern::new(head_height)?),
        StackItem::Box(head),
    ];
    let mut rows_before = add(add(result_height, head_height)?, head_depth)?;
    if let Some(working) = &working {
        items.push(StackItem::Kern(Kern::new(working_height)?));
        items.push(StackItem::Box(working.tree.clone()));
        rows_before = add(add(rows_before, working_height)?, working_depth)?;
    }
    // The dividend's baseline is the construct's baseline: it rides
    // `thickness*2 + rule` below the head's top.
    let dividend_baseline = add(
        add(result_height_px, thickness * 3.0)?,
        dividend.tree.metrics().baseline(),
    )?;
    let baseline = dividend_baseline.min(rows_before);
    let tree = BoxTree::vbox(items, baseline)?;

    // Glyphs place at their node offsets: the result row at the top, the
    // divisor left of the bracket, the dividend in its block, and the
    // working stack under the head.
    let divisor_x = 0.0;
    let dividend_x = add(add(divisor.tree.metrics().width(), pad * 2.0)?, thickness)?;
    let head_top = result_height_px;
    let mut glyphs = Vec::new();
    let placed_nodes: Vec<(Point, super::Node)> = vec![
        (Point::new(result_x.max(0.0), 0.0)?, result),
        (Point::new(divisor_x, head_top)?, divisor),
        (
            Point::new(dividend_x, add(head_top, thickness * 3.0)?)?,
            dividend,
        ),
    ];
    let working_offset = Point::new(0.0, add(head_top, head_total)?)?;
    for (offset, mut node) in placed_nodes {
        for glyph in &mut node.glyphs {
            glyph.origin = glyph.origin.translated(offset)?;
        }
        glyphs.append(&mut node.glyphs);
    }
    if let Some(mut stack) = working {
        for glyph in &mut stack.glyphs {
            glyph.origin = glyph.origin.translated(working_offset)?;
        }
        glyphs.append(&mut stack.glyphs);
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

fn empty() -> Node<'static> {
    Node {
        tree: BoxTree::space(Metrics::new(0.0, 0.0, 0.0).unwrap()),
        glyphs: Vec::new(),
        operator: None,
        operator_font_size_px: None,
        italic: 0.0,
        empty: true,
        right_space: None,
        max_size_percent: None,
    }
}
