//! Formula tokens and baseline-aligned rows with explicit per-call options.

mod baseline;
mod error;
mod fraction;
mod limits;
mod longdiv;
mod menclose;
mod operator;
mod radical;
mod rows;
mod scripts;
mod stack;
mod style;
mod table;
mod token;
mod variant;

pub use error::LayoutError;
pub use logifos_model::MathColor;
pub use style::{LayoutOptions, ResolvedStyle};

use crate::font::{FontProvider, ResolvedGlyph};
use crate::{BoxTree, Point};
use logifos_model::{Formula, LengthUnit, MAX_DEPTH, MAX_NODES};

/// One selected glyph, in input order, with its pen on the result's baseline.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedGlyph {
    pub character: char,
    pub glyph: ResolvedGlyph,
    /// Baseline origin in the root box's top-left coordinate system.
    pub origin: Point,
    pub font_size_px: f64,
    /// Explicit mathcolor in effect for this glyph; `None` inherits the paint.
    pub color: Option<logifos_model::MathColor>,
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
    /// Explicit rspace of the operator this node embellishes, in pixels.
    right_space: Option<f64>,
    /// The operator's stretch ceiling as a fraction of its natural extent.
    max_size_percent: Option<f64>,
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
    let mut accent = false;
    let mut max_size_percent = None;
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
        if let Some(space) = hints.right_space {
            // Core 3.2.4: rspace in 1/18 em of the operator's own size.
            let mut node = build_construct(content, fonts, style, budget, depth, false)?;
            let size = node.operator_font_size_px.unwrap_or(style.font_size_px);
            node.right_space = Some(size * space.units() / 18.0);
            return Ok(node);
        }
        accent = hints.accent == Some(true);
        max_size_percent = hints.max_size.and_then(|length| match length.unit {
            LengthUnit::Percent => Some(f64::from(length.value as i32) / 100.0),
            _ => None,
        });
        (content.as_ref(), style)
    } else {
        (formula, style)
    };
    if let Formula::Menclose { notation, content } = formula {
        let inner: &Formula = content;
        return menclose::enclose(notation, inner, fonts, style, budget, depth);
    }
    if let Formula::Stack(align, rows) = formula {
        return stack::stack(*align, rows, fonts, style, budget, depth);
    }
    if let Formula::LongDivision(division) = formula {
        return longdiv::long_division(division, fonts, style, budget, depth);
    }
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
        _ => {
            let mut node = build_construct(formula, fonts, style, budget, depth, accent)?;
            node.max_size_percent = max_size_percent;
            Ok(node)
        }
    }
}

/// Core 3.4.4: a script gains a math-depth level and compact displaystyle;
/// the subscript is also math-shift compact.
fn script_styles(
    fonts: &mut FontProvider,
    style: ResolvedStyle,
) -> Result<(ResolvedStyle, ResolvedStyle), LayoutError> {
    Ok((
        style.raise(fonts, 1, true)?,
        style.raise(fonts, 1, style.cramped)?,
    ))
}

/// An accent overscript gains the level but keeps the font size (Core 3.4.4).
fn over_style(
    fonts: &mut FontProvider,
    style: ResolvedStyle,
    accent: bool,
) -> Result<ResolvedStyle, LayoutError> {
    let raised = style.raise(fonts, 1, style.cramped)?;
    Ok(if accent {
        ResolvedStyle {
            font_size_px: style.font_size_px,
            ..raised
        }
    } else {
        raised
    })
}

/// Core 3.4.2.1: compact limits over an embellished operator with
/// `movablelimits` lay out as scripts.
fn movable(base: &Node<'_>, style: ResolvedStyle) -> bool {
    !style.display && base.operator.is_some_and(operator::movable_limits)
}

fn scripted_pair<'a>(
    base: Node<'a>,
    sub: Option<&'a Formula>,
    sup: Option<&'a Formula>,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
    budget: &mut Budget,
    depth: usize,
) -> Result<Node<'a>, LayoutError> {
    let (lowered, raised) = script_styles(fonts, style)?;
    let sub = sub
        .map(|child| build(child, fonts, lowered, budget, depth + 1))
        .transpose()?;
    let sup = sup
        .map(|child| build(child, fonts, raised, budget, depth + 1))
        .transpose()?;
    scripts::multi(
        base,
        vec![scripts::Pair { sub, sup }],
        Vec::new(),
        fonts,
        style,
    )
}

fn build_construct<'a>(
    formula: &'a Formula,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
    budget: &mut Budget,
    depth: usize,
    accent: bool,
) -> Result<Node<'a>, LayoutError> {
    match formula {
        Formula::Styled { .. }
        | Formula::Sequence(_)
        | Formula::Style(_)
        | Formula::ErrorGroup(_)
        | Formula::Padded(_)
        | Formula::Phantom(_)
        | Formula::Menclose { .. }
        | Formula::Stack(..) => unreachable!("handled by build"),
        Formula::LongDivision(_) => unreachable!("handled by build"),
        Formula::Space(width, height, depth) => {
            let size = style.font_size_px;
            let font = fonts.primary();
            let ex = font.x_height.map_or(size * 0.5, |units| {
                f64::from(units) * size / f64::from(font.metrics.units_per_em)
            });
            let px = |length: &Option<logifos_model::Length>| {
                length.as_ref().map_or(0.0, |l| l.px(size, ex)).max(0.0)
            };
            Ok(Node {
                operator_font_size_px: None,
                right_space: None,
                max_size_percent: None,
                tree: BoxTree::space(crate::Metrics::new(px(width), px(height), px(depth))?),
                glyphs: Vec::new(),
                operator: None,
                italic: 0.0,
                empty: false,
            })
        }
        Formula::StringLiteral(_) => Err(LayoutError::NotLaidOutYet),
        Formula::Symbol(text) => token::token(text, None, fonts, style, budget),
        Formula::Operator(text) => {
            operator::validate(text)?;
            token::token(text, Some(text), fonts, style, budget)
        }
        Formula::Fraction(..) | Formula::Bevelled(..) | Formula::Sqrt(_) | Formula::Root(..)
            if style.color.is_some() =>
        {
            // Rules carry no color, so a colored rule would paint wrongly.
            Err(LayoutError::NotLaidOutYet)
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
        Formula::Bevelled(numerator, denominator) => {
            let child_style = style.raise(fonts, u8::from(!style.display), style.cramped)?;
            let numerator = build(numerator, fonts, child_style, budget, depth + 1)?;
            let denominator = build(denominator, fonts, child_style, budget, depth + 1)?;
            fraction::bevelled(numerator, denominator, fonts, style)
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
        Formula::SubSup(base, sub, sup) => {
            let base = build(base, fonts, style, budget, depth + 1)?;
            scripted_pair(
                base,
                Some(sub.as_ref()),
                Some(sup.as_ref()),
                fonts,
                style,
                budget,
                depth,
            )
        }
        Formula::MultiScripts {
            base,
            postscripts,
            prescripts,
        } => {
            let base = build(base, fonts, style, budget, depth + 1)?;
            let mut script = |pairs: &'a [logifos_model::ScriptPair]| {
                pairs
                    .iter()
                    .map(|pair| {
                        let (lowered, raised) = script_styles(fonts, style)?;
                        let mut one = |child: &'a Option<Box<Formula>>, at| {
                            child
                                .as_deref()
                                .map(|child| build(child, fonts, at, budget, depth + 1))
                                .transpose()
                        };
                        Ok(scripts::Pair {
                            sub: one(&pair.subscript, lowered)?,
                            sup: one(&pair.superscript, raised)?,
                        })
                    })
                    .collect::<Result<Vec<_>, LayoutError>>()
            };
            let post = script(postscripts)?;
            let pre = script(prescripts.as_deref().unwrap_or(&[]))?;
            scripts::multi(base, post, pre, fonts, style)
        }
        Formula::Under(base, under) => {
            let base = build(base, fonts, style, budget, depth + 1)?;
            if movable(&base, style) {
                let lowered = style.raise(fonts, 1, true)?;
                let under = build(under, fonts, lowered, budget, depth + 1)?;
                return scripts::subscript(base, under, fonts, style);
            }
            let raised = style.raise(fonts, 1, style.cramped)?;
            let under = build(under, fonts, raised, budget, depth + 1)?;
            limits::limits(base, Some(under), None, false, fonts, style)
        }
        Formula::Over(base, over) => {
            let base_style = ResolvedStyle {
                cramped: style.cramped || accent,
                ..style
            };
            let base = build(base, fonts, base_style, budget, depth + 1)?;
            if movable(&base, style) {
                let raised = style.raise(fonts, 1, style.cramped)?;
                let over = build(over, fonts, raised, budget, depth + 1)?;
                return scripts::superscript(base, over, fonts, style);
            }
            let over_at = over_style(fonts, style, accent)?;
            let over = build(over, fonts, over_at, budget, depth + 1)?;
            limits::limits(base, None, Some(over), accent, fonts, style)
        }
        Formula::UnderOver(base, under, over) => {
            let base_style = ResolvedStyle {
                cramped: style.cramped || accent,
                ..style
            };
            let base = build(base, fonts, base_style, budget, depth + 1)?;
            if movable(&base, style) {
                return scripted_pair(
                    base,
                    Some(under.as_ref()),
                    Some(over.as_ref()),
                    fonts,
                    style,
                    budget,
                    depth,
                );
            }
            let raised = style.raise(fonts, 1, style.cramped)?;
            let under = build(under, fonts, raised, budget, depth + 1)?;
            let over_at = over_style(fonts, style, accent)?;
            let over = build(over, fonts, over_at, budget, depth + 1)?;
            limits::limits(base, Some(under), Some(over), accent, fonts, style)
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
        Formula::Table(rows, columnlines) => {
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
            table::table(built, *columnlines, fonts, style)
        }
    }
}
