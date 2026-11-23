use std::fmt;

/// A stable failure category for facade error mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// Source bytes are not UTF-8.
    InvalidUtf8,
    /// XML structure, namespaces, characters or references are malformed.
    MalformedXml,
    /// DTDs, custom entities or external resource facilities are prohibited.
    UnsafeXml,
    /// The document is outside the supported XML/MathML syntax boundary.
    UnsupportedSyntax,
    /// A source-byte, node-count or depth ceiling was exceeded.
    ResourceLimit,
    /// A caller supplied zero or raised a hard limit.
    InvalidLimits,
}

/// A bounded diagnostic that never includes caller source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Stable parser category.
    pub kind: ParseErrorKind,
    /// UTF-8 byte offset in the original source, including any BOM.
    pub offset: usize,
    /// Static diagnostic; not a stable assertion string.
    pub message: &'static str,
}

impl ParseError {
    pub(crate) fn new(kind: ParseErrorKind, offset: usize, message: &'static str) -> Self {
        Self {
            kind,
            offset,
            message,
        }
    }

    /// Maps parser categories to the facade's version 1 error vocabulary.
    pub fn code(&self) -> &'static str {
        match self.kind {
            ParseErrorKind::InvalidUtf8 => "INVALID_UTF8",
            ParseErrorKind::ResourceLimit => "RESOURCE_LIMIT",
            ParseErrorKind::InvalidLimits => "INVALID_REQUEST",
            ParseErrorKind::MalformedXml
            | ParseErrorKind::UnsafeXml
            | ParseErrorKind::UnsupportedSyntax => "UNSUPPORTED_SYNTAX",
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for ParseError {}
