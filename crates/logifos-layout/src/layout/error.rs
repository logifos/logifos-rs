use crate::font::FontError;
use crate::GeometryError;
use std::fmt;

/// Recognized Formula constructs whose geometry is not supported yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Construct {
    Superscript,
    Subscript,
}

/// Typed, all-or-nothing layout refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    UnsupportedConstruct(Construct),
    UnsupportedOperator(String),
    InvalidOptions,
    InvalidMathConstants,
    ResourceLimit,
    Font(FontError),
    Geometry(GeometryError),
}

impl LayoutError {
    /// Stable classification for a future facade adapter.
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedConstruct(_) | Self::UnsupportedOperator(_) => "NOT_IMPLEMENTED",
            Self::InvalidOptions => "INVALID_REQUEST",
            Self::InvalidMathConstants => "MISSING_METRICS",
            Self::ResourceLimit => "RESOURCE_LIMIT",
            Self::Font(error) => error.code(),
            Self::Geometry(_) => "INVALID_GEOMETRY",
        }
    }
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.code())?;
        match self {
            Self::UnsupportedConstruct(kind) => write!(f, "unsupported {kind:?}"),
            Self::UnsupportedOperator(_) => f.write_str("unsupported operator"),
            Self::InvalidOptions => f.write_str("invalid layout size"),
            Self::InvalidMathConstants => f.write_str("invalid script scale percentages"),
            Self::ResourceLimit => f.write_str("layout budget exceeded"),
            Self::Font(error) => error.fmt(f),
            Self::Geometry(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for LayoutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Font(error) => Some(error),
            Self::Geometry(error) => Some(error),
            _ => None,
        }
    }
}

impl From<FontError> for LayoutError {
    fn from(value: FontError) -> Self {
        Self::Font(value)
    }
}
impl From<GeometryError> for LayoutError {
    fn from(value: GeometryError) -> Self {
        Self::Geometry(value)
    }
}
