//! Formula tokens and baseline-aligned rows with explicit per-call options.

mod baseline;
mod error;
mod fraction;
mod operator;
mod radical;
mod rows;
mod scripts;
mod style;
mod table;
mod token;

pub use error::LayoutError;
pub use style::{LayoutOptions, ResolvedStyle};

use crate::font::{FontProvider, ResolvedGlyph};
use crate::{BoxTree, Point};
use logifos_model::{Formula, MAX_DEPTH, MAX_NODES};

/// One selected glyph, in input order, with its pen on the result's baseline.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedGlyph {
    pub character: char,
    pub glyph: ResolvedGlyph,
    /// Baseline origin in the root box's top-left coordinate system.
    pub origin: Point,
    pub font_size_px: f64,
}

/// Geometry plus the font and glyph information needed by a later painter.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub tree: BoxTree,
    pub glyphs: Vec<PlacedGlyph>,
    pub style: ResolvedStyle,
}

/// Layout never changes Formula, consults host fonts, or draws outlines.
/// Unsupported operators and unavailable glyph metrics refuse the whole call.
/// Limits also apply to directly constructed Formula values, not just wire input.
pub fn layout(
    formula: &Formula,
    fonts: &mut FontProvider,
    options: LayoutOptions,
) -> Result<Layout, LayoutError> {
    let style = options.resolve(fonts)?;
    let mut budget = Budget {
        nodes: 0,
        glyphs: 0,
    };
    let node = build(formula, fonts, style, &mut budget, 1)?;
    let style = if let Formula::Styled { style: hints, .. } = formula {
        style.presentation(*hints, fonts)?
    } else {
        style
    };
    let node = rows::finish(node, style)?;
    Ok(Layout {
        tree: node.tree,
        glyphs: node.glyphs,
        style,
    })
}

struct Node<'a> {
    tree: BoxTree,
    glyphs: Vec<PlacedGlyph>,
    operator: Option<&'a str>,
    operator_font_size_px: Option<f64>,
    italic: f64,
    empty: bool,
}

struct Budget {
    nodes: usize,
    glyphs: usize,
}

impl Budget {
    fn glyph(&mut self) -> Result<(), LayoutError> {
        self.glyphs += 1;
        if self.glyphs > MAX_NODES {
            return Err(LayoutError::ResourceLimit);
        }
        Ok(())
    }
}

fn build<'a>(
    formula: &'a Formula,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
    budget: &mut Budget,
    depth: usize,
) -> Result<Node<'a>, LayoutError> {
    budget.nodes += 1;
    if depth > MAX_DEPTH || budget.nodes > MAX_NODES {
        return Err(LayoutError::ResourceLimit);
    }
    let (formula, style) = if let Formula::Styled {
        style: hints,
        content,
    } = formula
    {
        let mut style = style;
        if !hints.valid_for(content, depth == 1, false) {
            return Err(LayoutError::InvalidOptions);
        }
        style = style.presentation(*hints, fonts)?;
        (content.as_ref(), style)
    } else {
        (formula, style)
    };
    match formula {
        Formula::Sequence(children)
        | Formula::Style(children)
        | Formula::ErrorGroup(children)
        | Formula::Padded(children)
        | Formula::Phantom(children) => {
            let children = children
                .iter()
                .map(|child| build(child, fonts, style, budget, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            let mut node = rows::row(children, fonts, style)?;
            if matches!(formula, Formula::Phantom(_)) {
                // Retain layout advances while removing glyph and rule ink.
                node.tree = BoxTree::space(node.tree.metrics());
                node.glyphs.clear();
            }
            Ok(node)
        }
        _ => build_construct(formula, fonts, style, budget, depth),
    }
}

fn build_construct<'a>(
    formula: &'a Formula,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
    budget: &mut Budget,
    depth: usize,
) -> Result<Node<'a>, LayoutError> {
    match formula {
        Formula::Styled { .. }
        | Formula::Sequence(_)
        | Formula::Style(_)
        | Formula::ErrorGroup(_)
        | Formula::Padded(_)
        | Formula::Phantom(_) => unreachable!("handled by build"),
        Formula::Under(..)
        | Formula::Over(..)
        | Formula::UnderOver(..)
        | Formula::SubSup(..)
        | Formula::MultiScripts { .. }
        | Formula::Space
        | Formula::StringLiteral(_) => Err(LayoutError::NotLaidOutYet),
        Formula::Symbol(text) => token::token(text, None, fonts, style, budget),
        Formula::Operator(text) => {
            operator::validate(text)?;
            token::token(text, Some(text), fonts, style, budget)
        }
        Formula::Fraction(numerator, denominator) => {
            // Core 3.3.2: compact children, auto-add depth only when the
            // fraction is already compact, and a cramped denominator.
            let child_style = style.raise(fonts, u8::from(!style.display), style.cramped)?;
            let numerator = build(numerator, fonts, child_style, budget, depth + 1)?;
            let denominator = build(
                denominator,
                fonts,
                ResolvedStyle {
                    cramped: true,
                    ..child_style
                },
                budget,
                depth + 1,
            )?;
            fraction::fraction(numerator, denominator, fonts, style)
        }
        Formula::Superscript(base, script) => {
            let base = build(base, fonts, style, budget, depth + 1)?;
            let raised = style.raise(fonts, 1, style.cramped)?;
            let script = build(script, fonts, raised, budget, depth + 1)?;
            scripts::superscript(base, script, fonts, style)
        }
        Formula::Subscript(base, script) => {
            let base = build(base, fonts, style, budget, depth + 1)?;
            let lowered = style.raise(fonts, 1, true)?;
            let script = build(script, fonts, lowered, budget, depth + 1)?;
            scripts::subscript(base, script, fonts, style)
        }
        Formula::Sqrt(radicand) => {
            // msqrt and mroot set math-shift compact on themselves and so on
            // their whole content (Core 3.3.3 stylesheet rules).
            let style = ResolvedStyle {
                cramped: true,
                ..style
            };
            let radicand = build(radicand, fonts, style, budget, depth + 1)?;
            radical::sqrt(radicand, fonts, style)
        }
        Formula::Root(radicand, index) => {
            let style = ResolvedStyle {
                cramped: true,
                ..style
            };
            let radicand = build(radicand, fonts, style, budget, depth + 1)?;
            // The index gains two levels (Core 3.3.3 stylesheet rules).
            let raised = style.raise(fonts, 2, true)?;
            let index = build(index, fonts, raised, budget, depth + 1)?;
            radical::root(radicand, index, fonts, style)
        }
        Formula::Table(rows) => {
            // mtable sets math-style compact (Core 3.5 stylesheet rules).
            let style = ResolvedStyle {
                display: false,
                ..style
            };
            let mut built = Vec::with_capacity(rows.len());
            for row in rows {
                let mut cells = Vec::with_capacity(row.len());
                for cell in row {
                    let cell = match cell {
                        Formula::Styled {
                            style: hints,
                            content,
                        } => {
                            if !hints.valid_for(content, false, true) {
                                return Err(LayoutError::InvalidOptions);
                            }
                            // Explicit center matches the table's existing alignment.
                            content.as_ref()
                        }
                        _ => cell,
                    };
                    cells.push(build(cell, fonts, style, budget, depth + 1)?);
                }
                built.push(cells);
            }
            table::table(built, fonts, style)
        }
    }
}
