//! Baseline shifts: ink extents and re-baselined wrappers for scripts and
//! constructs that move a child off the row baseline.

use crate::geometry::add;
use crate::{BoxTree, Kern, StackItem};

/// The ink's rise above the baseline; 0 for a box without ink.
pub(super) fn ink_ascent(tree: &BoxTree) -> f64 {
    tree.ink_bounds()
        .map_or(0.0, |b| tree.metrics().baseline() - b.origin().y())
}

/// The ink's drop below the baseline; 0 for a box without ink.
pub(super) fn ink_descent(tree: &BoxTree) -> f64 {
    tree.ink_bounds().map_or(0.0, |b| {
        b.origin().y() + b.height() - tree.metrics().baseline()
    })
}

/// Wraps a box whose baseline rises by `shift` above the row baseline,
/// reporting the box's own y offset inside the wrapper.
pub(super) fn raised(tree: BoxTree, shift: f64) -> Result<(BoxTree, f64), crate::GeometryError> {
    let height = tree.metrics().height();
    let depth = tree.metrics().depth();
    let pad = (shift - depth).max(0.0);
    let items = if pad > 0.0 {
        vec![StackItem::Box(tree), StackItem::Kern(Kern::new(pad)?)]
    } else {
        vec![StackItem::Box(tree)]
    };
    // height + shift and the stack total are the same value only up to the
    // association order; the min keeps the baseline inside the extent.
    let baseline = (height + shift).min(add(add(height, depth)?, pad)?);
    let wrapper = BoxTree::vbox(items, baseline)?;
    Ok((wrapper, 0.0))
}

/// Wraps a box whose baseline drops by `shift` below the row baseline,
/// reporting the box's own y offset inside the wrapper.
pub(super) fn lowered(tree: BoxTree, shift: f64) -> Result<(BoxTree, f64), crate::GeometryError> {
    let height = tree.metrics().height();
    let pad = (shift - height).max(0.0);
    let items = if pad > 0.0 {
        vec![StackItem::Kern(Kern::new(pad)?), StackItem::Box(tree)]
    } else {
        vec![StackItem::Box(tree)]
    };
    let wrapper = BoxTree::vbox(items, (height - shift).max(0.0))?;
    Ok((wrapper, pad))
}
