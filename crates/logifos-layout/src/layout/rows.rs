use super::{operator, LayoutError, Node, ResolvedStyle};
use crate::{BoxTree, Kern, StackItem};

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
    style: ResolvedStyle,
) -> Result<Node<'a>, LayoutError> {
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

pub(super) fn finish(node: Node<'_>, style: ResolvedStyle) -> Result<Node<'_>, LayoutError> {
    let space = node.operator.map_or(0.0, |text| {
        operator::space(text, false, false, style.font_size_px)
    });
    let trailing = node.italic;
    stack(vec![(node, space, space + trailing)])
}
