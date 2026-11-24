//! Default-instance TrueType bounds, independent of drawable path decoding.

use super::error::Result;
use super::reader::Reader;
use super::{FontError, FontErrorKind, FontFace, FontProvider, ResolvedGlyph};
use crate::{Bounds, Metrics};
use fontisan::sfnt::{OutlineKind, Outlines};

/// Control-point bounds in design units, with font coordinates increasing up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphBounds {
    pub x_min: i16,
    pub y_min: i16,
    pub x_max: i16,
    pub y_max: i16,
}

pub(super) fn read(outlines: &Outlines<'_>) -> Result<Option<Vec<Option<GlyphBounds>>>> {
    if outlines.kind != OutlineKind::Glyf {
        return Ok(None);
    }
    outlines
        .glyphs
        .iter()
        .map(|bytes| {
            if bytes.is_empty() {
                return Ok(None);
            }
            let r = Reader::new(bytes, "glyf");
            r.range(0, 10, 1)?;
            let contours = r.i16(0)?;
            if contours < -1 {
                return Err(r.err(0));
            }
            if contours == 0 {
                return Ok(None);
            }
            let bounds = GlyphBounds {
                x_min: r.i16(2)?,
                y_min: r.i16(4)?,
                x_max: r.i16(6)?,
                y_max: r.i16(8)?,
            };
            if bounds.x_min > bounds.x_max || bounds.y_min > bounds.y_max {
                return Err(r.err(2));
            }
            Ok(Some(bounds))
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

impl FontFace {
    /// `None` means a valid contourless glyph; unavailable CFF bounds refuse.
    pub fn glyph_bounds(&self, glyph: u16) -> Result<Option<GlyphBounds>> {
        self.glyph_metrics(glyph)?;
        self.bounds
            .as_ref()
            .ok_or_else(|| FontError::new(FontErrorKind::MetricsUnavailable))?
            .get(usize::from(glyph))
            .copied()
            .ok_or_else(|| FontError::new(FontErrorKind::MissingGlyph))
    }
}

/// A glyph's logical metrics and enclosing ink rectangle in top-left coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasuredGlyph {
    pub glyph: ResolvedGlyph,
    pub metrics: Metrics,
    pub ink_bounds: Option<Bounds>,
}

impl FontProvider {
    /// Uses the first covering resource; missing bounds never silently select
    /// another font or substitute font-wide ascent/descent for glyph bounds.
    pub fn measure(&mut self, character: char, font_size_px: f64) -> Result<MeasuredGlyph> {
        let glyph = self.resolve(character, font_size_px)?;
        let font = &self
            .faces
            .iter()
            .find(|(id, _)| *id == glyph.resource_id)
            .ok_or_else(|| FontError::new(FontErrorKind::InvalidResources))?
            .1;
        measured_glyph(font, glyph, font_size_px)
    }
}

/// Derives a glyph's logical metrics and ink rectangle from its face bounds.
pub(super) fn measured_glyph(
    font: &FontFace,
    glyph: ResolvedGlyph,
    font_size_px: f64,
) -> Result<MeasuredGlyph> {
    let bounds = font.glyph_bounds(glyph.glyph_id)?;
    let mut height = 0.0;
    let mut depth = 0.0;
    let ink_bounds = if let Some(b) = bounds {
        let top = font.scale(f64::from(b.y_max), font_size_px)?;
        let bottom = font.scale(f64::from(b.y_min), font_size_px)?;
        height = top.max(0.0);
        depth = (-bottom).max(0.0);
        Some(
            Bounds::new(
                glyph.left_side_bearing_px,
                height - top,
                font.scale(f64::from(b.x_max) - f64::from(b.x_min), font_size_px)?,
                top - bottom,
            )
            .map_err(|_| FontError::new(FontErrorKind::MetricsUnavailable))?,
        )
    } else {
        None
    };
    let metrics = Metrics::new(glyph.advance_px, height, depth)
        .map_err(|_| FontError::new(FontErrorKind::MetricsUnavailable))?;
    Ok(MeasuredGlyph {
        glyph,
        metrics,
        ink_bounds,
    })
}
