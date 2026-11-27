//! MathML-3 enclosures: the content draws inside rule-drawn borders. The
//! box family and the line rules draw as filled rules; circle draws a
//! stroked ellipse; actuarial and phasorangle join their rules with a
//! corner arc. An empty notation draws nothing (the MathML 4 default), and
//! a list containing an unsupported spelling refuses typed so the gap
//! surfaces.

use super::{build, LayoutError, Node, ResolvedStyle};
use crate::font::math::MathConstant;
use crate::font::FontProvider;
use crate::{ArcCorner, BoxKind, BoxTree, Metrics, Point, PositionedBox};
use logifos_model::Formula;

pub(super) fn enclose<'a>(
    notation: &str,
    content: &'a Formula,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
    budget: &mut Budget,
    depth: usize,
) -> Result<Node<'a>, LayoutError> {
    let names: Vec<&str> = notation.split_whitespace().collect();
    // MathML 4: no notation, or an empty string, draws nothing.
    if names.is_empty() {
        return build(content, fonts, style, budget, depth);
    }
    let draws: Vec<Enclosure> = names
        .iter()
        .map(|name| Enclosure::parse(name))
        .collect::<Option<Vec<_>>>()
        .ok_or(LayoutError::NotLaidOutYet)?;
    let thickness =
        fonts.math_constant_px(MathConstant::FractionRuleThickness, style.font_size_px)?;
    let pad = thickness;
    let inner = build(content, fonts, style, budget, depth + 1)?;
    let inner_w = inner.tree.metrics().width();
    let inner_h = inner.tree.metrics().height();
    let inner_d = inner.tree.metrics().depth();
    let width = inner_w + pad * 2.0;
    let height = inner_h + pad;
    let depth_out = inner_d + pad;
    let metrics = Metrics::new(width, height, depth_out)?;
    let total = height + depth_out;

    let mut children = Vec::new();
    children.push(PositionedBox::place(Point::new(pad, pad)?, inner.tree));
    for draw in &draws {
        draw.draw(width, total, thickness, &mut children)?;
    }
    let tree = BoxTree::composed(BoxKind::HBox, metrics, children)?;

    let mut glyphs = inner.glyphs;
    for glyph in &mut glyphs {
        glyph.origin = glyph.origin.translated(Point::new(pad, pad)?)?;
    }
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

/// One understood enclosure spelling, drawn independently of the others.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Enclosure {
    Box,
    Top,
    Bottom,
    Left,
    Right,
    Circle,
    Actuarial,
    Phasorangle,
}

impl Enclosure {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "box" => Some(Self::Box),
            "top" => Some(Self::Top),
            "bottom" => Some(Self::Bottom),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "circle" => Some(Self::Circle),
            "actuarial" => Some(Self::Actuarial),
            "phasorangle" => Some(Self::Phasorangle),
            _ => None,
        }
    }

    fn draw(
        self,
        width: f64,
        total: f64,
        thickness: f64,
        children: &mut Vec<PositionedBox>,
    ) -> Result<(), LayoutError> {
        match self {
            Self::Box => {
                children.push(rule(0.0, 0.0, width, thickness)?);
                children.push(rule(0.0, total - thickness, width, thickness)?);
                children.push(rule(0.0, thickness, thickness, total - thickness * 2.0)?);
                children.push(rule(
                    width - thickness,
                    thickness,
                    thickness,
                    total - thickness * 2.0,
                )?);
            }
            Self::Top => children.push(rule(0.0, 0.0, width, thickness)?),
            Self::Bottom => children.push(rule(0.0, total - thickness, width, thickness)?),
            Self::Left => children.push(rule(0.0, 0.0, thickness, total)?),
            Self::Right => children.push(rule(width - thickness, 0.0, thickness, total)?),
            Self::Circle => {
                let metrics = Metrics::new(width, total, 0.0)?;
                children.push(PositionedBox::place(
                    Point::new(0.0, 0.0)?,
                    BoxTree::ellipse(metrics, thickness)?,
                ));
            }
            // The actuarial symbol encloses the top and the right, with the
            // shared corner rounded: the arc is centered on the rounded
            // band's inner corner and bulges out to the top right.
            Self::Actuarial => {
                let radius = self.arc_radius(thickness);
                children.push(rule(0.0, 0.0, width - radius, thickness)?);
                children.push(rule(width - thickness, radius, thickness, total - radius)?);
                let metrics = Metrics::new(radius, radius, 0.0)?;
                children.push(PositionedBox::place(
                    Point::new(width - radius, 0.0)?,
                    BoxTree::arc(ArcCorner::BottomLeft, metrics, thickness)?,
                ));
            }
            // The phasor angle is a bottom rule with the angle's arc rising
            // from the baseline's left corner.
            Self::Phasorangle => {
                let radius = self.arc_radius(thickness);
                children.push(rule(0.0, total - thickness, width, thickness)?);
                let metrics = Metrics::new(radius, radius, 0.0)?;
                children.push(PositionedBox::place(
                    Point::new(0.0, total - radius)?,
                    BoxTree::arc(ArcCorner::BottomLeft, metrics, thickness)?,
                ));
            }
        }
        Ok(())
    }

    /// The rounded-corner radius: a multiple of the rule thickness so the
    /// arc reads as a corner and not a seam.
    fn arc_radius(self, thickness: f64) -> f64 {
        match self {
            Self::Actuarial | Self::Phasorangle => thickness * 3.0,
            _ => thickness,
        }
    }
}

fn rule(x: f64, y: f64, width: f64, thickness: f64) -> Result<PositionedBox, LayoutError> {
    Ok(PositionedBox::place(
        Point::new(x, y)?,
        BoxTree::rule(width, thickness, 0.0)?,
    ))
}

use super::Budget;
