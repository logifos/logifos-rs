use std::fmt;

/// A font refusal that can be mapped to the facade's structured error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontErrorKind {
    InvalidFont,
    UnsupportedFormat,
    MissingTable,
    MalformedTable,
    MissingGlyph,
    HashMismatch,
    InvalidResources,
    ResourceLimit,
    InvalidSize,
    OutlinesUnavailable,
}

/// Diagnostics contain a table tag and byte offset, never caller font bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontError {
    pub kind: FontErrorKind,
    pub table: Option<&'static str>,
    pub offset: Option<usize>,
}

impl FontError {
    pub(super) fn new(kind: FontErrorKind) -> Self {
        Self {
            kind,
            table: None,
            offset: None,
        }
    }

    pub(super) fn malformed(table: &'static str, offset: usize) -> Self {
        Self {
            kind: FontErrorKind::MalformedTable,
            table: Some(table),
            offset: Some(offset),
        }
    }

    /// Stable wire code for a future facade integration.
    pub fn code(&self) -> &'static str {
        match self.kind {
            FontErrorKind::MissingGlyph => "MISSING_GLYPH",
            FontErrorKind::HashMismatch => "FONT_HASH_MISMATCH",
            FontErrorKind::ResourceLimit => "RESOURCE_LIMIT",
            FontErrorKind::InvalidResources | FontErrorKind::InvalidSize => "INVALID_REQUEST",
            FontErrorKind::OutlinesUnavailable => "NOT_IMPLEMENTED",
            _ => "INVALID_FONT",
        }
    }
}

impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}", self.code(), self.kind)?;
        if let Some(table) = self.table {
            write!(f, " in {table} at byte {}", self.offset.unwrap_or(0))?;
        }
        Ok(())
    }
}

impl std::error::Error for FontError {}

pub(super) type Result<T> = std::result::Result<T, FontError>;
