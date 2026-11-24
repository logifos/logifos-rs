//! Formula tokens and baseline-aligned rows with explicit per-call options.

mod error;
mod fraction;
mod operator;
mod rows;
mod scripts;
mod style;
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
    match formula {
        Formula::Sequence(children) => {
            let children = children
                .iter()
                .map(|child| build(child, fonts, style, budget, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            rows::row(children, style)
        }
        Formula::Symbol(text) => token::token(text, None, fonts, style, budget),
        Formula::Operator(text) => {
            operator::validate(text)?;
            token::token(text, Some(text), fonts, style, budget)
        }
        Formula::Fraction(numerator, denominator) => {
            let numerator = build(numerator, fonts, style, budget, depth + 1)?;
            let denominator = build(denominator, fonts, style, budget, depth + 1)?;
            fraction::fraction(numerator, denominator, fonts, style)
        }
        Formula::Superscript(base, script) => {
            let base = build(base, fonts, style, budget, depth + 1)?;
            let raised = style.raise(fonts, style.cramped)?;
            let script = build(script, fonts, raised, budget, depth + 1)?;
            scripts::superscript(base, script, fonts, style)
        }
        Formula::Subscript(base, script) => {
            let base = build(base, fonts, style, budget, depth + 1)?;
            let lowered = style.raise(fonts, true)?;
            let script = build(script, fonts, lowered, budget, depth + 1)?;
            scripts::subscript(base, script, fonts, style)
        }
    }
}
