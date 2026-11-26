//! Immutable boxes and explicit ordered stacking operations.

use crate::geometry::{add, extent, finite, subtract};
use crate::{Bounds, GeometryError, Metrics, Point};

/// A finite signed gap in CSS pixels. Negative gaps move the stack cursor back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kern(f64);

impl Kern {
    /// Validates a signed gap, normalizing negative zero.
    pub fn new(amount: f64) -> Result<Self, GeometryError> {
        Ok(Self(finite(amount)?))
    }

    /// The signed cursor movement.
    pub fn amount(self) -> f64 {
        self.0
    }
}

/// One input to a stack; a kern applies along that stack's axis.
#[derive(Debug, Clone, PartialEq)]
pub enum StackItem {
    /// A child to place at the current cursor.
    Box(BoxTree),
    /// Cursor movement with no ink or cross-axis extent.
    Kern(Kern),
}

impl From<BoxTree> for StackItem {
    fn from(value: BoxTree) -> Self {
        Self::Box(value)
    }
}

impl From<Kern> for StackItem {
    fn from(value: Kern) -> Self {
        Self::Kern(value)
    }
}

/// The geometric role of a box; resolved presentation stays within layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxKind {
    /// An externally measured leaf with optional ink.
    Leaf,
    /// A filled rectangle covering the logical box.
    Rule,
    /// A baseline-aligned horizontal stack.
    HBox,
    /// A left-aligned vertical stack with an explicit parent baseline.
    VBox,
}

/// A child's top-left position relative to its parent's top-left origin.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionedBox {
    offset: Point,
    node: BoxTree,
}

impl PositionedBox {
    /// Places a child at a top-left offset over the parent's logical box.
    pub fn place(offset: Point, node: BoxTree) -> Self {
        Self { offset, node }
    }

    /// The child's local-to-parent translation.
    pub fn offset(&self) -> Point {
        self.offset
    }

    /// The placed child, retaining its own baseline and local ink bounds.
    pub fn node(&self) -> &BoxTree {
        &self.node
    }
}

/// A validated geometric tree, independent of syntax, font loading, and output.
///
/// Constructors consume signed kerns into child positions and final advance.
/// Intermediate cursor positions can be negative; final logical dimensions
/// must be nonnegative. Ink union retains overhangs and overlapping children.
/// Empty stacks have no ink. Child order is input order with kerns removed.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxTree {
    kind: BoxKind,
    metrics: Metrics,
    ink_bounds: Option<Bounds>,
    children: Vec<PositionedBox>,
}

impl BoxTree {
    /// Constructs a measured leaf; ink may overhang any logical edge.
    pub fn leaf(metrics: Metrics, ink_bounds: Option<Bounds>) -> Self {
        Self {
            kind: BoxKind::Leaf,
            metrics,
            ink_bounds,
            children: Vec::new(),
        }
    }

    /// Constructs an inkless logical spacer.
    pub fn space(metrics: Metrics) -> Self {
        Self::leaf(metrics, None)
    }

    /// Constructs a filled rule, extending from y=0 through height plus depth.
    /// A rule with either zero width or zero total height has no ink.
    pub fn rule(width: f64, height: f64, depth: f64) -> Result<Self, GeometryError> {
        let metrics = Metrics::new(width, height, depth)?;
        let ink_bounds = if metrics.width() == 0.0 || metrics.total_height() == 0.0 {
            None
        } else {
            Some(Bounds::new(
                0.0,
                0.0,
                metrics.width(),
                metrics.total_height(),
            )?)
        };
        Ok(Self {
            kind: BoxKind::Rule,
            ..Self::leaf(metrics, ink_bounds)
        })
    }

    /// Places children left-to-right, aligning every child baseline.
    /// Width is the ordered sum of advances and kerns; height and depth are
    /// the maximum child height and depth, respectively.
    pub fn hbox(items: Vec<StackItem>) -> Result<Self, GeometryError> {
        let mut width = 0.0;
        let mut height: f64 = 0.0;
        let mut depth: f64 = 0.0;
        let mut children = Vec::new();
        for item in items {
            match item {
                StackItem::Kern(kern) => width = add(width, kern.amount())?,
                StackItem::Box(node) => {
                    let offset = Point::new(width, 0.0)?;
                    width = add(width, node.metrics.width())?;
                    height = height.max(node.metrics.height());
                    depth = depth.max(node.metrics.depth());
                    children.push(PositionedBox { offset, node });
                }
            }
        }
        let metrics = Metrics::new(width, height, depth)?;
        for child in &mut children {
            child.offset = Point::new(
                child.offset.x(),
                subtract(height, child.node.metrics.height())?,
            )?;
        }
        Self::stack(BoxKind::HBox, metrics, children)
    }

    /// Stacks children top-to-bottom, with their left edges at x=0.
    ///
    /// The baseline is measured down from the parent origin and must be within
    /// the final vertical advance. It does not realign child baselines. Width
    /// is the maximum child advance; total height is the ordered sum of child
    /// vertical extents and signed kerns. An empty stack accepts baseline zero.
    pub fn vbox(items: Vec<StackItem>, baseline: f64) -> Result<Self, GeometryError> {
        let baseline = extent(baseline)?;
        let mut width: f64 = 0.0;
        let mut total = 0.0;
        let mut children = Vec::new();
        for item in items {
            match item {
                StackItem::Kern(kern) => total = add(total, kern.amount())?,
                StackItem::Box(node) => {
                    let offset = Point::new(0.0, total)?;
                    total = add(total, node.metrics.total_height())?;
                    width = width.max(node.metrics.width());
                    children.push(PositionedBox { offset, node });
                }
            }
        }
        let total = extent(total)?;
        if baseline > total {
            return Err(GeometryError::BaselineOutOfBounds);
        }
        let metrics = Metrics::new(width, baseline, subtract(total, baseline)?)?;
        Self::stack(BoxKind::VBox, metrics, children)
    }

    /// A custom composition: children positioned absolutely over the
    /// parent's logical box (an overlay such as an enclosure's border);
    /// ink unions the translated children.
    pub fn composed(
        kind: BoxKind,
        metrics: Metrics,
        children: Vec<PositionedBox>,
    ) -> Result<Self, GeometryError> {
        Self::stack(kind, metrics, children)
    }

    fn stack(
        kind: BoxKind,
        metrics: Metrics,
        children: Vec<PositionedBox>,
    ) -> Result<Self, GeometryError> {
        let mut ink: Option<Bounds> = None;
        for child in &children {
            if let Some(bounds) = child.node.ink_bounds {
                let bounds = bounds.translated(child.offset)?;
                ink = Some(match ink {
                    None => bounds,
                    Some(previous) => previous.union(bounds)?,
                });
            }
        }
        Ok(Self {
            kind,
            metrics,
            ink_bounds: ink,
            children,
        })
    }

    /// The box's geometric role.
    pub fn kind(&self) -> BoxKind {
        self.kind
    }

    /// Logical advance metrics in CSS pixels.
    pub fn metrics(&self) -> Metrics {
        self.metrics
    }

    /// Union of actual ink in this box's coordinate system, without clipping.
    pub fn ink_bounds(&self) -> Option<Bounds> {
        self.ink_bounds
    }

    /// Positioned children in stable input order.
    pub fn children(&self) -> &[PositionedBox] {
        &self.children
    }
}
