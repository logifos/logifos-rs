//! Elementary stacks: rows of cells stacked vertically with a shared
//! column grid, per MathML-3's mstack. Columns are cell-granular in this
//! pass: one column is the stack's widest cell, positions pad in that
//! unit, and the decimal-point alignment rides right-alignment; the
//! per-character column pass is future work.

use super::{build, Budget, LayoutError, Node, ResolvedStyle};
use crate::font::math::MathConstant;
use crate::font::FontProvider;
use crate::geometry::{add, subtract};
use crate::{BoxTree, Kern, Point, StackItem};
use logifos_model::{Formula, StackAlign, StackRow};

pub(super) fn stack<'a>(
    align: StackAlign,
    rows: &'a [StackRow],
    fonts: &mut FontProvider,
    style: ResolvedStyle,
    budget: &mut Budget,
    depth: usize,
) -> Result<Node<'a>, LayoutError> {
    // Groups expand up front: the flat list carries only cell rows and
    // rules, with every group position resolved into an effective cell
    // position.
    let mut expanded = Vec::new();
    expand_rows(rows, None, None, &mut expanded);
    let mut built = Vec::with_capacity(expanded.len());
    for row in &expanded {
        let Flat::Cells { cells, .. } = row else {
            built.push(None);
            continue;
        };
        let mut items = Vec::with_capacity(cells.len());
        let mut cells_out = Vec::with_capacity(cells.len());
        for cell in cells {
            let node = build(cell, fonts, style, budget, depth + 1)?;
            items.push(StackItem::Box(node.tree.clone()));
            cells_out.push(node);
        }
        built.push(Some((BoxTree::hbox(items)?, cells_out)));
    }

    // The column unit and the stack width: the widest cell drives the
    // position padding, and the widest shifted row drives the width.
    let unit = built
        .iter()
        .flatten()
        .flat_map(|(_, cells)| cells.iter())
        .map(|node| node.tree.metrics().width())
        .fold(0.0f64, f64::max);
    let mut width = 0.0f64;
    for (row, laid) in expanded.iter().zip(&built) {
        let position = match row {
            Flat::Cells { position, .. } => *position,
            Flat::Line { position, .. } => *position,
        };
        let extent = match laid {
            Some((tree, _)) => add(tree.metrics().width(), position_shift(position, unit).abs())?,
            None => width,
        };
        width = width.max(extent);
    }

    // Rule rows draw after the grid pass so they span the final width.
    let thickness =
        fonts.math_constant_px(MathConstant::FractionRuleThickness, style.font_size_px)?;
    let mut items = Vec::new();
    let mut glyphs = Vec::new();
    let mut rows_before = 0.0;
    let mut last_depth = 0.0f64;
    let mut saw_row = false;
    for (row, laid) in expanded.iter().zip(&mut built) {
        match row {
            Flat::Line { position, length } => {
                let shift = position_shift(*position, unit);
                let rule_width = match length {
                    Some(columns) => f64::from(*columns) * unit,
                    None => subtract(width, shift.abs())?,
                };
                items.push(StackItem::Kern(Kern::new(thickness)?));
                items.push(StackItem::Box(BoxTree::rule(rule_width, thickness, 0.0)?));
                rows_before = add(add(rows_before, thickness)?, thickness)?;
                last_depth = 0.0;
            }
            Flat::Cells { position, .. } => {
                let Some((tree, cells)) = laid.take() else {
                    continue;
                };
                let height = tree.metrics().height();
                let depth = tree.metrics().depth();
                let shift = position_shift(*position, unit);
                let x = match align {
                    StackAlign::Left => (-shift).max(0.0),
                    StackAlign::Center => {
                        add(subtract(width, tree.metrics().width())? / 2.0, shift)?
                    }
                    StackAlign::Right | StackAlign::DecimalPoint => {
                        subtract(subtract(width, tree.metrics().width())?, shift)?
                    }
                };
                // The shift rides the row's own leading kern so the logical
                // boxes carry it, not just the glyphs.
                let row_tree = BoxTree::hbox(vec![
                    StackItem::Kern(Kern::new(x)?),
                    StackItem::Box(tree.clone()),
                ])?;
                // The row's baseline rides at the bottom of its height; each
                // cell's glyphs translate by the cell's in-row offset.
                let y = add(rows_before, height)?;
                let mut cell_x = x;
                for mut cell in cells {
                    let cell_offset = Point::new(cell_x, y)?;
                    for glyph in &mut cell.glyphs {
                        glyph.origin = glyph.origin.translated(cell_offset)?;
                    }
                    glyphs.append(&mut cell.glyphs);
                    cell_x = add(cell_x, cell.tree.metrics().width())?;
                }
                items.push(StackItem::Kern(Kern::new(height)?));
                items.push(StackItem::Box(row_tree));
                rows_before = add(add(rows_before, height)?, depth)?;
                last_depth = depth;
                saw_row = true;
            }
        }
    }
    let baseline = if saw_row {
        subtract(rows_before, last_depth)?
    } else {
        0.0
    };
    let tree = BoxTree::vbox(items, baseline)?;

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

/// One expanded stack row: cells with their effective position, or a rule.
enum Flat<'a> {
    Cells {
        position: Option<i32>,
        cells: Vec<&'a Formula>,
    },
    Line {
        position: Option<i32>,
        length: Option<i32>,
    },
}

/// Expands group nesting into the flat row list: a row's own position wins,
/// otherwise the group's position plus the per-row shift accumulates.
fn expand_rows<'a>(
    rows: &'a [StackRow],
    group_position: Option<i32>,
    shift: Option<i32>,
    out: &mut Vec<Flat<'a>>,
) {
    for (index, row) in rows.iter().enumerate() {
        match row {
            StackRow::Line { position, length } => out.push(Flat::Line {
                position: *position,
                length: *length,
            }),
            StackRow::Cells { position, cells } => {
                let effective = position.or(group_position.map(|base| {
                    base + i32::try_from(index).unwrap_or(i32::MAX) * shift.unwrap_or_default()
                }));
                out.push(Flat::Cells {
                    position: effective,
                    cells: cells.iter().collect(),
                });
            }
            StackRow::Group {
                position,
                shift: inner_shift,
                rows: group,
            } => {
                expand_rows(
                    group,
                    position.or(group_position),
                    inner_shift.or(shift),
                    out,
                );
            }
        }
    }
}

/// The pixel shift of a position: positive pads columns on the right (the
/// row moves left), negative pads on the left.
fn position_shift(position: Option<i32>, unit: f64) -> f64 {
    position.map_or(0.0, |position| f64::from(position) * unit)
}
