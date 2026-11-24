//! Font-independent, deterministic SVG emission from a validated box tree and
//! its placed glyphs. The [`outline`] module exposes typed move, line,
//! quadratic, cubic, and close commands and their SVG path serializer.
//!
//! Rules become filled rectangles; placed glyphs become `<path>` data (TrueType
//! quadratics with implied on-curve midpoints, flipped into top-left
//! coordinates at their baseline origin). Boxes paint first, the glyph layer
//! paints last. A glyph with no decodable contours paints nothing: loading a
//! glyf face refuses any glyph that claims ink bounds without contours, so a
//! missing outline can never hide measured ink. This crate neither loads
//! fonts nor substitutes text or bounding rectangles for glyphs.
//!
//! The viewport includes both logical advance and ink overhangs. Its dimensions
//! in CSS pixels equal the viewBox extents, preserving one user unit per pixel.
//! Logical metrics and ink bounds remain in the original layout coordinates;
//! the viewBox origin explicitly records any viewport padding on the top/left.
//!
//! Element/attribute order and preorder `box-N` IDs are fixed; the glyph layer
//! is a trailing `glyphs` group in glyph order. Numbers use Rust's
//! locale-independent, shortest round-trip binary64 Display representation,
//! with negative zero normalized and no rounding or quantization. Equal trees,
//! glyph lists and options produce equal bytes with the same engine artifact.

pub mod outline;

use std::fmt;

use outline::{GlyphOutline, OutlineError};

use logifos_layout::layout::PlacedGlyph;
use logifos_layout::{Bounds, BoxKind, BoxTree, GeometryError, Metrics, Point};

/// Per-emission presentation. No CSS, URLs, or raw XML are accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgOptions {
    /// A six-digit RGB hex color (`#RRGGBB`), emitted in lowercase.
    pub color: String,
    /// Optional plain-text SVG title, escaped as XML 1.0 character data.
    pub title: Option<String>,
}

impl Default for SvgOptions {
    fn default() -> Self {
        Self {
            color: "#000000".into(),
            title: None,
        }
    }
}

/// An emission refusal. Errors never contain a partially emitted document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgError {
    /// Color must contain exactly `#` followed by six ASCII hex digits.
    InvalidColor,
    /// The title contains a character forbidden by XML 1.0.
    InvalidXmlCharacter,
    /// Combining logical and ink extents exceeded representable geometry.
    Geometry(GeometryError),
    /// Invalid outline commands, non-finite points, or glyph placement overflow.
    Outline(OutlineError),
}

impl fmt::Display for SvgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidColor => f.write_str("SVG color must be a six-digit RGB hex string"),
            Self::InvalidXmlCharacter => f.write_str("SVG title contains an invalid XML character"),
            Self::Geometry(error) => write!(f, "SVG viewport: {error}"),
            Self::Outline(error) => write!(f, "SVG {error}"),
        }
    }
}

impl std::error::Error for SvgError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Geometry(error) => Some(error),
            Self::Outline(error) => Some(error),
            _ => None,
        }
    }
}

impl From<GeometryError> for SvgError {
    fn from(value: GeometryError) -> Self {
        Self::Geometry(value)
    }
}

/// A complete self-contained SVG plus unchanged layout geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct SvgDocument {
    svg: String,
    metrics: Metrics,
    ink_bounds: Option<Bounds>,
    viewport: Bounds,
}

impl SvgDocument {
    /// UTF-8 SVG markup, without an XML declaration or trailing newline.
    pub fn as_str(&self) -> &str {
        &self.svg
    }

    /// Logical advance, baseline height, and depth in CSS pixels.
    pub fn metrics(&self) -> Metrics {
        self.metrics
    }

    /// Ink bounds in the original top-left layout coordinate system.
    pub fn ink_bounds(&self) -> Option<Bounds> {
        self.ink_bounds
    }

    /// The viewBox in layout coordinates; its extents are the pixel dimensions.
    /// Empty/zero-area trees retain zero extents and paint no ink.
    pub fn viewport(&self) -> Bounds {
        self.viewport
    }
}

/// Emits rules, inkless leaves, stacks and placed glyphs in stable input order.
///
/// No external resources, host fonts, scripts, styles, or event handlers appear
/// in the output. Traversal uses an explicit stack rather than recursive calls.
/// Geometry validation remains owned by `logifos-layout`; a contourless glyph
/// contributes no path data.
///
/// # Errors
///
/// Refuses invalid presentation, outline geometry, and arithmetic overflow.
/// No partial SVG is returned.
///
/// # Examples
///
/// ```
/// use logifos_layout::BoxTree;
/// use logifos_svg::{emit, SvgOptions};
///
/// let rule = BoxTree::rule(8.0, 0.5, 0.0)?;
/// let document = emit(&rule, &[], &SvgOptions::default())?;
/// assert_eq!(document.metrics().width(), 8.0);
/// assert!(document.as_str().contains("<rect"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn emit(
    tree: &BoxTree,
    glyphs: &[PlacedGlyph],
    options: &SvgOptions,
) -> Result<SvgDocument, SvgError> {
    if options.color.len() != 7
        || !options.color.starts_with('#')
        || !options.color.as_bytes()[1..]
            .iter()
            .all(u8::is_ascii_hexdigit)
    {
        return Err(SvgError::InvalidColor);
    }
    if let Some(title) = &options.title {
        if !title.chars().all(is_xml_character) {
            return Err(SvgError::InvalidXmlCharacter);
        }
    }

    let metrics = tree.metrics();
    let logical = Bounds::new(0.0, 0.0, metrics.width(), metrics.total_height())?;
    let viewport = match tree.ink_bounds() {
        Some(ink) => logical.union(ink)?,
        None => logical,
    };
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}px\" height=\"{}px\" viewBox=\"{} {} {} {}\">",
        number(viewport.width()),
        number(viewport.height()),
        number(viewport.origin().x()),
        number(viewport.origin().y()),
        number(viewport.width()),
        number(viewport.height()),
    );
    if let Some(title) = &options.title {
        svg.push_str("<title>");
        escape_text(title, &mut svg);
        svg.push_str("</title>");
    }
    svg.push_str(&format!(
        "<g fill=\"{}\">",
        options.color.to_ascii_lowercase()
    ));

    enum Visit<'a> {
        Open(&'a BoxTree, Point, bool),
        Close,
    }
    let mut pending = vec![Visit::Open(tree, Point::default(), false)];
    let mut id = 0_usize;
    while let Some(visit) = pending.pop() {
        let Visit::Open(node, offset, hidden) = visit else {
            svg.push_str("</g>");
            continue;
        };
        let hidden = hidden || node.kind() == BoxKind::Phantom;
        svg.push_str(&format!(
            "<g id=\"box-{id}\" transform=\"translate({} {})\">",
            number(offset.x()),
            number(offset.y()),
        ));
        id += 1;
        if !hidden && node.kind() == BoxKind::Rule && node.ink_bounds().is_some() {
            svg.push_str(&format!(
                "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\"/>",
                number(node.metrics().width()),
                number(node.metrics().total_height()),
            ));
        }
        pending.push(Visit::Close);
        for child in node.children().iter().rev() {
            pending.push(Visit::Open(child.node(), child.offset(), hidden));
        }
    }
    let layer = glyph_layer(glyphs)?;
    if !layer.is_empty() {
        svg.push_str("<g id=\"glyphs\">");
        svg.push_str(&layer);
        svg.push_str("</g>");
    }
    svg.push_str("</g></svg>");
    Ok(SvgDocument {
        svg,
        metrics,
        ink_bounds: tree.ink_bounds(),
        viewport,
    })
}

fn number(value: f64) -> String {
    if value == 0.0 {
        "0".into()
    } else {
        value.to_string()
    }
}

/// One `<path>` per placed glyph with contours, in glyph order. Font contour
/// decoding is separate from command serialization and final placement.
fn glyph_layer(glyphs: &[PlacedGlyph]) -> Result<String, SvgError> {
    let mut layer = String::new();
    for glyph in glyphs {
        let Some(outline) = &glyph.glyph.outline_px else {
            continue;
        };
        let outline = GlyphOutline::try_from(outline).map_err(SvgError::Outline)?;
        let data = outline
            .to_svg_path(glyph.origin)
            .map_err(SvgError::Outline)?;
        if !data.is_empty() {
            layer.push_str(&format!("<path d=\"{data}\"/>"));
        }
    }
    Ok(layer)
}

fn is_xml_character(value: char) -> bool {
    matches!(value, '\u{9}' | '\u{a}' | '\u{d}' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}

fn escape_text(text: &str, output: &mut String) {
    for value in text.chars() {
        match value {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&apos;"),
            '\r' => output.push_str("&#13;"),
            _ => output.push(value),
        }
    }
}
