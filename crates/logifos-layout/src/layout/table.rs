//! Tables: a column grid of baseline-aligned rows, centered on the math
//! axis per MathML Core section 3.5's inline-table semantics.

use super::{LayoutError, Node, ResolvedStyle};
use crate::font::math::MathConstant;
use crate::font::FontProvider;
use crate::geometry::{add, subtract};
use crate::{BoxTree, Kern, Metrics, Point, StackItem};

/// The em and ex shares Core's mtd stylesheet applies as cell padding.
const HORIZONTAL_PAD_EM: f64 = 0.4;
const VERTICAL_PAD_EX: f64 = 0.5;
/// CSS fallback for the ex unit when the face carries no OS/2 sxHeight.
const EX_EM_FALLBACK: f64 = 0.5;

pub(super) fn table<'a>(
    rows: Vec<Vec<Node<'a>>>,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
) -> Result<Node<'a>, LayoutError> {
    let size = style.font_size_px;
    let font = fonts.primary();
    let ex = match font.x_height {
        Some(units) => f64::from(units) * size / f64::from(font.metrics.units_per_em),
        None => EX_EM_FALLBACK * size,
    };
    let horizontal_pad = HORIZONTAL_PAD_EM * size;
    let vertical_pad = VERTICAL_PAD_EX * ex;
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);

    // Grid pass: padded extents fix the column widths and row extents;
    // ragged rows leave trailing cells empty.
    let mut column_widths = vec![0.0f64; columns];
    let mut row_extents = Vec::with_capacity(rows.len());
    for row in &rows {
        let mut widths = Vec::with_capacity(columns);
        let mut height = 0.0f64;
        let mut depth = 0.0f64;
        for (column, node) in row
            .iter()
            .map(Some)
            .chain(std::iter::repeat_n(None, columns))
            .take(columns)
            .enumerate()
        {
            let (width, cell_height, cell_depth) = node.map_or((0.0, 0.0, 0.0), |node| {
                (
                    node.tree.metrics().width(),
                    node.tree.metrics().height(),
                    node.tree.metrics().depth(),
                )
            });
            let padded = add(add(width, horizontal_pad)?, horizontal_pad)?;
            // The cell wrappers below carry both pads; the grid extents take
            // each side once.
            height = height.max(add(cell_height, vertical_pad)?);
            depth = depth.max(add(cell_depth, vertical_pad)?);
            column_widths[column] = column_widths[column].max(padded);
            widths.push(padded);
        }
        row_extents.push((widths, height, depth));
    }

    // Row pass: cells center per column with the padding riding the kerns,
    // and every glyph records its final in-table offset directly.
    let mut row_trees = Vec::with_capacity(rows.len());
    let mut glyphs = Vec::new();
    let mut rows_before = 0.0;
    for (row, (widths, height, depth)) in rows.into_iter().zip(row_extents.iter()) {
        let mut cells = row.into_iter();
        let mut items = Vec::with_capacity(columns * 3);
        let mut leading = 0.0;
        for column in 0..columns {
            let gap = add(
                subtract(column_widths[column], widths[column])? / 2.0,
                horizontal_pad,
            )?;
            items.push(StackItem::Kern(Kern::new(gap)?));
            let x = add(leading, gap)?;
            let cell = cells.next();
            let cell_height = cell
                .as_ref()
                .map_or(0.0, |node| node.tree.metrics().height());
            let y = add(rows_before, subtract(*height, cell_height)?)?;
            let offset = Point::new(x, y)?;
            match cell {
                Some(mut node) => {
                    let baseline = add(vertical_pad, node.tree.metrics().height())?;
                    for glyph in &mut node.glyphs {
                        glyph.origin = glyph.origin.translated(offset)?;
                    }
                    glyphs.extend(node.glyphs);
                    // The cell's padding rides a wrapper whose baseline keeps
                    // the content on the row baseline while its extents grow
                    // by the pad on both sides.
                    items.push(StackItem::Box(BoxTree::vbox(
                        vec![
                            StackItem::Kern(Kern::new(vertical_pad)?),
                            StackItem::Box(node.tree),
                            StackItem::Kern(Kern::new(vertical_pad)?),
                        ],
                        baseline,
                    )?));
                }
                None => {
                    items.push(StackItem::Box(BoxTree::space(Metrics::new(0.0, 0.0, 0.0)?)));
                }
            }
            items.push(StackItem::Kern(Kern::new(gap)?));
            leading = add(leading, column_widths[column])?;
        }
        row_trees.push(BoxTree::hbox(items)?);
        rows_before = add(add(rows_before, *height)?, *depth)?;
    }

    // The table's center rides the math axis; a table smaller than twice the
    // axis floats above the baseline inside a wrapper carrying the gap.
    let total = rows_before;
    let axis = fonts.math_constant_px(MathConstant::AxisHeight, size)?;
    let baseline = add(total / 2.0, axis)?;
    let items = row_trees.into_iter().map(StackItem::Box).collect();
    let tree = if baseline <= total {
        BoxTree::vbox(items, baseline)?
    } else {
        let inner = BoxTree::vbox(items, total)?;
        BoxTree::vbox(
            vec![
                StackItem::Box(inner),
                StackItem::Kern(Kern::new(baseline - total)?),
            ],
            baseline,
        )?
    };

    Ok(Node {
        tree,
        glyphs,
        operator: None,
        italic: 0.0,
        empty: false,
    })
}
