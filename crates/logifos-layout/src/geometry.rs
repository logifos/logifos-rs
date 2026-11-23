//! Validated binary64 coordinates, logical metrics, and ink rectangles.

use std::fmt;

/// A rejected input or unrepresentable geometry operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryError {
    /// An input was NaN or infinite.
    NonFinite,
    /// A logical dimension or rectangle extent was negative.
    NegativeExtent,
    /// Arithmetic on finite values exceeded the finite binary64 range.
    Overflow,
    /// A vertical stack's baseline was outside its logical vertical extent.
    BaselineOutOfBounds,
}

impl fmt::Display for GeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NonFinite => "geometry input must be finite",
            Self::NegativeExtent => "geometry extent must be nonnegative",
            Self::Overflow => "geometry arithmetic exceeded finite binary64 range",
            Self::BaselineOutOfBounds => "baseline must lie within the vertical stack",
        })
    }
}

impl std::error::Error for GeometryError {}

pub(crate) fn finite(value: f64) -> Result<f64, GeometryError> {
    if !value.is_finite() {
        return Err(GeometryError::NonFinite);
    }
    Ok(if value == 0.0 { 0.0 } else { value })
}

pub(crate) fn extent(value: f64) -> Result<f64, GeometryError> {
    let value = finite(value)?;
    if value < 0.0 {
        Err(GeometryError::NegativeExtent)
    } else {
        Ok(value)
    }
}

fn arithmetic(value: f64) -> Result<f64, GeometryError> {
    finite(value).map_err(|_| GeometryError::Overflow)
}

pub(crate) fn add(left: f64, right: f64) -> Result<f64, GeometryError> {
    arithmetic(left + right)
}

pub(crate) fn subtract(left: f64, right: f64) -> Result<f64, GeometryError> {
    arithmetic(left - right)
}

/// A finite coordinate or displacement in CSS pixels; either axis may be negative.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    x: f64,
    y: f64,
}

impl Point {
    /// Constructs a coordinate, normalizing negative zero.
    pub fn new(x: f64, y: f64) -> Result<Self, GeometryError> {
        Ok(Self {
            x: finite(x)?,
            y: finite(y)?,
        })
    }

    /// The horizontal coordinate.
    pub fn x(self) -> f64 {
        self.x
    }

    /// The vertical coordinate.
    pub fn y(self) -> f64 {
        self.y
    }

    /// Adds a displacement with checked arithmetic.
    pub fn translated(self, offset: Self) -> Result<Self, GeometryError> {
        Self::new(add(self.x, offset.x)?, add(self.y, offset.y)?)
    }
}

/// An ink rectangle in the containing box's top-left coordinate system.
///
/// Zero-sized rectangles are allowed and retained by union. Absence of ink is
/// represented separately by `None`, never by a rectangle at the origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    origin: Point,
    width: f64,
    height: f64,
}

impl Bounds {
    /// Constructs a rectangle whose origin, extents, and far edges are finite.
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Result<Self, GeometryError> {
        let origin = Point::new(x, y)?;
        let width = extent(width)?;
        let height = extent(height)?;
        add(x, width)?;
        add(y, height)?;
        Ok(Self {
            origin,
            width,
            height,
        })
    }

    /// The top-left corner, which may be outside the logical box.
    pub fn origin(self) -> Point {
        self.origin
    }

    /// The nonnegative horizontal ink extent.
    pub fn width(self) -> f64 {
        self.width
    }

    /// The nonnegative vertical ink extent.
    pub fn height(self) -> f64 {
        self.height
    }

    /// Moves ink without clipping it to the logical box.
    pub fn translated(self, offset: Point) -> Result<Self, GeometryError> {
        let origin = self.origin.translated(offset)?;
        Self::new(origin.x, origin.y, self.width, self.height)
    }

    /// Returns the smallest axis-aligned rectangle containing both rectangles.
    pub fn union(self, other: Self) -> Result<Self, GeometryError> {
        let x = self.origin.x.min(other.origin.x);
        let y = self.origin.y.min(other.origin.y);
        let right = add(self.origin.x, self.width)?.max(add(other.origin.x, other.width)?);
        let bottom = add(self.origin.y, self.height)?.max(add(other.origin.y, other.height)?);
        Self::new(x, y, subtract(right, x)?, subtract(bottom, y)?)
    }
}

/// Nonnegative logical advance, height above baseline, and depth below it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Metrics {
    width: f64,
    height: f64,
    depth: f64,
}

impl Metrics {
    /// Constructs metrics, also checking that `height + depth` is finite.
    pub fn new(width: f64, height: f64, depth: f64) -> Result<Self, GeometryError> {
        let width = extent(width)?;
        let height = extent(height)?;
        let depth = extent(depth)?;
        add(height, depth)?;
        Ok(Self {
            width,
            height,
            depth,
        })
    }

    /// Horizontal advance in CSS pixels.
    pub fn width(self) -> f64 {
        self.width
    }

    /// Distance from the logical top to the baseline.
    pub fn height(self) -> f64 {
        self.height
    }

    /// Distance below the baseline.
    pub fn depth(self) -> f64 {
        self.depth
    }

    /// Baseline y coordinate; always equal to `height`.
    pub fn baseline(self) -> f64 {
        self.height
    }

    /// Logical vertical extent, validated at construction.
    pub fn total_height(self) -> f64 {
        self.height + self.depth
    }
}
