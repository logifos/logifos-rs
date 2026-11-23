//! Deterministic box geometry in CSS pixels.
//!
//! A box's origin is the top-left of its logical advance area. Positive x goes
//! right and positive y goes down. Its baseline is `height` below the origin;
//! `depth` extends below the baseline. Ink bounds can overhang that area.
//!
//! All public constructors reject nonfinite values and invalid extents.
//! Arithmetic uses binary64 in input order, checks every intermediate result,
//! and canonicalizes signed zero. No quantization or font-unit conversion is
//! performed here. Equal inputs produce equal geometry without hidden state.

pub mod boxes;
pub mod font;
pub mod geometry;
pub mod layout;

pub use boxes::{BoxKind, BoxTree, Kern, PositionedBox, StackItem};
pub use geometry::{Bounds, GeometryError, Metrics, Point};
