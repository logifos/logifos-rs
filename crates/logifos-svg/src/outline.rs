//! Typed glyph paths, independent of a font's contour encoding.
//!
//! Coordinates are CSS pixels in glyph-local font orientation (y up). The
//! serializer places every endpoint and control point at a baseline origin,
//! flipping y once. It emits absolute SVG commands without curve conversion,
//! rounding, or contour reordering.

use std::fmt;

use logifos_layout::font::{ContourPx, GlyphOutlinePx};
use logifos_layout::{GeometryError, Point};

/// One absolute command in a glyph-local, y-up outline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathCommand {
    MoveTo(Point),
    LineTo(Point),
    QuadTo {
        control: Point,
        to: Point,
    },
    CubicTo {
        control1: Point,
        control2: Point,
        to: Point,
    },
    Close,
}

/// An invalid contour sequence or unrepresentable glyph coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineError {
    /// Every contour must start with a move and end with a close. Drawing
    /// outside a contour, nested moves, and unclosed contours are refused.
    InvalidCommandOrder,
    Geometry(GeometryError),
}

impl fmt::Display for OutlineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCommandOrder => f.write_str("glyph outline contours must be closed"),
            Self::Geometry(error) => write!(f, "glyph outline: {error}"),
        }
    }
}

impl std::error::Error for OutlineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Geometry(error) => Some(error),
            Self::InvalidCommandOrder => None,
        }
    }
}

impl From<GeometryError> for OutlineError {
    fn from(value: GeometryError) -> Self {
        Self::Geometry(value)
    }
}

/// Ordered, closed glyph contours with finite coordinates.
///
/// An empty outline is valid and serializes to an empty string. Quadratic and
/// cubic curves may coexist; no font decoder is required to construct a path.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GlyphOutline {
    commands: Vec<PathCommand>,
}

impl GlyphOutline {
    /// Validates contour order without changing any commands or coordinates.
    pub fn new(commands: Vec<PathCommand>) -> Result<Self, OutlineError> {
        let mut open = false;
        for command in &commands {
            match command {
                PathCommand::MoveTo(_) if !open => open = true,
                PathCommand::Close if open => open = false,
                PathCommand::LineTo(_)
                | PathCommand::QuadTo { .. }
                | PathCommand::CubicTo { .. }
                    if open => {}
                _ => return Err(OutlineError::InvalidCommandOrder),
            }
        }
        if open {
            return Err(OutlineError::InvalidCommandOrder);
        }
        Ok(Self { commands })
    }

    pub fn commands(&self) -> &[PathCommand] {
        &self.commands
    }

    /// Emits SVG path data at a top-left layout baseline origin.
    ///
    /// Commands use `M`, `L`, `Q`, `C`, `Z`, with one space between numeric
    /// operands and after each drawing command. `Z` has no trailing space;
    /// the next contour follows immediately. Numbers use the same shortest
    /// round-trip binary64 formatting as the SVG document; negative zero is
    /// normalized. Finite placement overflow returns an error, never partial
    /// path data.
    pub fn to_svg_path(&self, origin: Point) -> Result<String, OutlineError> {
        let mut data = String::new();
        let place = |point: Point| -> Result<String, OutlineError> {
            let x = origin.x() + point.x();
            let y = origin.y() - point.y();
            if !x.is_finite() || !y.is_finite() {
                return Err(GeometryError::Overflow.into());
            }
            Ok(format!("{} {}", super::number(x), super::number(y)))
        };
        for command in &self.commands {
            match *command {
                PathCommand::MoveTo(to) => data.push_str(&format!("M{} ", place(to)?)),
                PathCommand::LineTo(to) => data.push_str(&format!("L{} ", place(to)?)),
                PathCommand::QuadTo { control, to } => {
                    data.push_str(&format!("Q{} {} ", place(control)?, place(to)?));
                }
                PathCommand::CubicTo {
                    control1,
                    control2,
                    to,
                } => data.push_str(&format!(
                    "C{} {} {} ",
                    place(control1)?,
                    place(control2)?,
                    place(to)?
                )),
                PathCommand::Close => data.push('Z'),
            }
        }
        Ok(data)
    }
}

/// Consumes the layout backend's Fontisan-decoded, pixel-scaled TrueType
/// points. Consecutive off-curve points imply quadratic midpoints, including
/// across the contour wrap. Empty and single-point contours paint nothing.
impl TryFrom<&GlyphOutlinePx> for GlyphOutline {
    type Error = OutlineError;

    fn try_from(outline: &GlyphOutlinePx) -> Result<Self, Self::Error> {
        let mut commands = Vec::new();
        for contour in &outline.contours {
            append_contour(&mut commands, contour)?;
        }
        Self::new(commands)
    }
}

fn append_contour(
    commands: &mut Vec<PathCommand>,
    contour: &ContourPx,
) -> Result<(), OutlineError> {
    let points = contour
        .points
        .iter()
        .map(|p| Ok((Point::new(p.x, p.y)?, p.on_curve)))
        .collect::<Result<Vec<_>, OutlineError>>()?;
    if points.len() < 2 {
        return Ok(());
    }
    if contour.cubic {
        // Type 2 contours start on the charstring moveto and alternate
        // on-curve anchors with off-off-on cubic segments; lines surface as
        // plain on-curve points.
        commands.push(PathCommand::MoveTo(points[0].0));
        let mut i = 1;
        while i < points.len() {
            if points[i].1 {
                commands.push(PathCommand::LineTo(points[i].0));
                i += 1;
            } else {
                let Some(&(control1, _)) = points.get(i) else {
                    return Err(OutlineError::InvalidCommandOrder);
                };
                let Some(&(control2, _)) = points.get(i + 1) else {
                    return Err(OutlineError::InvalidCommandOrder);
                };
                let Some(&(to, true)) = points.get(i + 2) else {
                    return Err(OutlineError::InvalidCommandOrder);
                };
                commands.push(PathCommand::CubicTo {
                    control1,
                    control2,
                    to,
                });
                i += 3;
            }
        }
        commands.push(PathCommand::Close);
        return Ok(());
    }
    let mut expanded = Vec::with_capacity(points.len());
    for (i, &(point, on_curve)) in points.iter().enumerate() {
        expanded.push((point, on_curve));
        let (next, next_on_curve) = points[(i + 1) % points.len()];
        if !on_curve && !next_on_curve {
            expanded.push((
                Point::new(point.x().midpoint(next.x()), point.y().midpoint(next.y()))?,
                true,
            ));
        }
    }
    // At least one original or implied on-curve point exists after expansion.
    let start = expanded.iter().position(|&(_, on)| on).unwrap();
    commands.push(PathCommand::MoveTo(expanded[start].0));
    let mut i = (start + 1) % expanded.len();
    while i != start {
        let (point, on_curve) = expanded[i];
        if on_curve {
            commands.push(PathCommand::LineTo(point));
            i = (i + 1) % expanded.len();
        } else {
            let next = (i + 1) % expanded.len();
            commands.push(PathCommand::QuadTo {
                control: point,
                to: expanded[next].0,
            });
            if next == start {
                break;
            }
            i = (next + 1) % expanded.len();
        }
    }
    commands.push(PathCommand::Close);
    Ok(())
}
