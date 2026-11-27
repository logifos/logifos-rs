use super::radical;
use super::{operator, LayoutError, Node, ResolvedStyle};
use crate::font::math::Direction;
use crate::font::FontProvider;
use crate::geometry::add;
use crate::{BoxTree, Kern, Metrics, StackItem};

pub(super) fn stack(parts: Vec<(Node<'_>, f64, f64)>) -> Result<Node<'_>, LayoutError> {
    let empty = parts.iter().all(|(node, _, _)| node.empty);
    let mut glyph_runs = Vec::new();
    let mut items = Vec::new();
    for (node, before, after) in parts {
        items.push(StackItem::Kern(Kern::new(before)?));
        items.push(StackItem::Box(node.tree));
        items.push(StackItem::Kern(Kern::new(after)?));
        glyph_runs.push(node.glyphs);
    }
    let tree = BoxTree::hbox(items)?;
    let mut glyphs = Vec::new();
    for (run, child) in glyph_runs.into_iter().zip(tree.children()) {
        for mut glyph in run {
            glyph.origin = glyph.origin.translated(child.offset())?;
            glyphs.push(glyph);
        }
    }
    Ok(Node {
        tree,
        glyphs,
        operator: None,
        operator_font_size_px: None,
        italic: 0.0,
        empty,
        right_space: None,
        max_size_percent: None,
    })
}

pub(super) fn row<'a>(
    children: Vec<Node<'a>>,
    fonts: &mut FontProvider,
    style: ResolvedStyle,
) -> Result<Node<'a>, LayoutError> {
    // Core 3.3.1.1: the fences (stretchy operators with block stretch axis)
    // re-lay at the other children's ink extents; with only fences present
    // the constraint is (0, 0).
    let mut ink_ascent = 0.0f64;
    let mut ink_descent = 0.0f64;
    for node in &children {
        if node.operator.is_some_and(operator::is_fence) {
            continue;
        }
        ink_ascent = ink_ascent.max(super::baseline::ink_ascent(&node.tree));
        ink_descent = ink_descent.max(super::baseline::ink_descent(&node.tree));
    }
    let children = children
        .into_iter()
        .map(
            |node| match node.operator.filter(|t| operator::is_fence(t)) {
                Some(text) => {
                    let color = node.glyphs.first().and_then(|glyph| glyph.color);
                    let size = node.operator_font_size_px.unwrap_or(style.font_size_px);
                    // Core 3.2.4: maxsize caps the stretch as a fraction of
                    // the operator's natural extent.
                    let cap = node.max_size_percent.map(|fraction| {
                        let ch = text.chars().next().unwrap_or('(');
                        fonts
                            .measure(ch, size)
                            .map(|natural| {
                                let ink = natural.ink_bounds.map_or(0.0, |bounds| bounds.height());
                                ink * fraction
                            })
                            .unwrap_or(0.0)
                    });
                    let mut stretched =
                        stretched_fence(text, ink_ascent, ink_descent, fonts, size, color, cap)?;
                    // Invisible embellished operators still stretch and space,
                    // but re-layout must not restore their painted output.
                    if node.tree.ink_bounds().is_none() && node.glyphs.is_empty() {
                        stretched.tree = BoxTree::space(stretched.tree.metrics());
                        stretched.glyphs.clear();
                    }
                    Ok(stretched)
                }
                None => Ok(node),
            },
        )
        .collect::<Result<Vec<_>, LayoutError>>()?;
    let count = children.iter().filter(|node| !node.empty).count();
    let core = if count == 1 {
        children
            .iter()
            .find(|node| !node.empty)
            .and_then(|node| node.operator)
    } else {
        None
    };
    let core_space = if core.is_some() {
        children
            .iter()
            .find(|node| !node.empty)
            .and_then(|node| node.right_space)
    } else {
        None
    };
    let core_style = if core.is_some() {
        children
            .iter()
            .find(|node| !node.empty)
            .and_then(|node| node.operator_font_size_px)
    } else {
        None
    };
    let mut previous_italic = 0.0;
    let mut index = 0;
    let mut parts = Vec::new();
    for node in children {
        let (lead, trail) = if core.is_none() {
            node.operator.map_or((0.0, 0.0), |text| {
                operator::space(
                    text,
                    operator::form(index, count),
                    node.operator_font_size_px.unwrap_or(style.font_size_px),
                    node.right_space,
                )
            })
        } else {
            (0.0, 0.0)
        };
        let correction = if !node.empty && node.italic == 0.0 {
            previous_italic
        } else {
            0.0
        };
        if !node.empty {
            previous_italic = node.italic;
            index += 1;
        }
        parts.push((node, correction + lead, trail));
    }
    let mut node = stack(parts)?;
    node.operator = core;
    node.operator_font_size_px = core_style;
    node.right_space = core_space;
    node.italic = previous_italic;
    Ok(node)
}

/// Re-lays one fence as a vertically stretched glyph box spanning the row's
/// ink extents, replacing its natural-size token.
fn stretched_fence<'a>(
    text: &'a str,
    ascent: f64,
    descent: f64,
    fonts: &mut FontProvider,
    font_size_px: f64,
    color: Option<crate::layout::MathColor>,
    cap: Option<f64>,
) -> Result<Node<'a>, LayoutError> {
    let ch = text.chars().next().unwrap_or('(');
    let target = match cap {
        Some(cap) => add(ascent, descent)?.min(cap),
        None => add(ascent, descent)?,
    };
    let stretched = fonts.expand_stretch(ch, Direction::Vertical, target, font_size_px)?;
    let tree = BoxTree::leaf(
        Metrics::new(stretched.advance_px, ascent.max(0.0), descent.max(0.0))?,
        None,
    );
    let glyphs = radical::stretched_pieces(&stretched, ch, 0.0, font_size_px, color)?;
    Ok(Node {
        tree,
        glyphs,
        operator: Some(text),
        operator_font_size_px: Some(font_size_px),
        italic: 0.0,
        empty: false,
        right_space: None,
        max_size_percent: None,
    })
}

pub(super) fn finish(node: Node<'_>, style: ResolvedStyle) -> Result<Node<'_>, LayoutError> {
    let (lead, trail) = node.operator.map_or((0.0, 0.0), |text| {
        operator::space(
            text,
            operator::form(0, 1),
            node.operator_font_size_px.unwrap_or(style.font_size_px),
            node.right_space,
        )
    });
    let trailing = node.italic;
    stack(vec![(node, lead, trail + trailing)])
}
