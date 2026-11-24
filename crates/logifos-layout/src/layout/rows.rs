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
        italic: 0.0,
        empty,
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
                Some(text) => stretched_fence(text, ink_ascent, ink_descent, fonts, style),
                None => Ok(node),
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let count = children.iter().filter(|node| !node.empty).count();
    let core = if count == 1 {
        children
            .iter()
            .find(|node| !node.empty)
            .and_then(|node| node.operator)
    } else {
        None
    };
    let mut previous_italic = 0.0;
    let mut index = 0;
    let mut parts = Vec::new();
    for node in children {
        let space = if core.is_none() {
            node.operator.map_or(0.0, |text| {
                operator::space(text, index == 0, count > 1, style.font_size_px)
            })
        } else {
            0.0
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
        parts.push((node, correction + space, space));
    }
    let mut node = stack(parts)?;
    node.operator = core;
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
    style: ResolvedStyle,
) -> Result<Node<'a>, LayoutError> {
    let ch = text.chars().next().unwrap_or('(');
    let stretched = fonts.expand_stretch(
        ch,
        Direction::Vertical,
        add(ascent, descent)?,
        style.font_size_px,
    )?;
    let tree = BoxTree::leaf(
        Metrics::new(stretched.advance_px, ascent.max(0.0), descent.max(0.0))?,
        None,
    );
    let glyphs = radical::stretched_pieces(&stretched, ch, 0.0, style.font_size_px)?;
    Ok(Node {
        tree,
        glyphs,
        operator: Some(text),
        italic: 0.0,
        empty: false,
    })
}

pub(super) fn finish(node: Node<'_>, style: ResolvedStyle) -> Result<Node<'_>, LayoutError> {
    let space = node.operator.map_or(0.0, |text| {
        operator::space(text, false, false, style.font_size_px)
    });
    let trailing = node.italic;
    stack(vec![(node, space, space + trailing)])
}
