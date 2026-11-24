//! Caller-supplied fonts loaded by Fontisan, with checked metrics and MATH data.
//! No filesystem access, bundled fonts, host fallback, or shared mutable cache.

mod bounds;
mod error;
pub mod math;
mod reader;
mod validate;

pub use bounds::{GlyphBounds, MeasuredGlyph};
use error::Result;
pub use error::{FontError, FontErrorKind};
use fontisan::sfnt::{Font, GlyphOutline, Options, OutlineKind};
use math::{Direction, GlyphAssembly, GlyphVariant, MathConstant, MathTable};
use reader::Reader;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const MAX_FONT_BYTES: usize = 4_194_304;
pub const MAX_FONTS: usize = 16;

/// Full file identity; separate faces of a collection remain separate resources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontIdentity {
    pub sha256: [u8; 32],
    pub face_index: u16,
}

/// Decoded request resource. The JSON boundary owns base64 and JSON decoding.
#[derive(Debug, Clone, Copy)]
pub struct FontResource<'a> {
    pub id: &'a str,
    pub sha256: &'a str,
    pub bytes: &'a [u8],
    pub face_index: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontMetrics {
    pub units_per_em: u16,
    pub ascent: i16,
    pub descent: i16,
    pub line_gap: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphMetrics {
    pub advance: u16,
    pub left_side_bearing: i16,
}

/// Metrics and MATH for one face, with decoded TrueType outline points.
#[derive(Debug)]
pub struct FontFace {
    pub identity: FontIdentity,
    pub metrics: FontMetrics,
    pub math: MathTable,
    mappings: Vec<(u32, u32)>,
    advances: Vec<GlyphMetrics>,
    bounds: Option<Vec<Option<GlyphBounds>>>,
    /// Per-glyph contour points in font units; `None` for a contourless glyph.
    outlines: Option<Vec<Option<GlyphOutline>>>,
}

impl FontFace {
    /// Accepts sfnt TTF/OTF and TTC/OTC bytes at the default variation instance.
    pub fn load(bytes: &[u8], face_index: u16) -> Result<Self> {
        if bytes.len() > MAX_FONT_BYTES {
            return Err(FontError::new(FontErrorKind::ResourceLimit));
        }
        validate::directory(bytes, face_index)?;
        // Contain failures in dependency readers as well as returning their errors.
        std::panic::catch_unwind(|| Self::read(bytes, face_index))
            .map_err(|_| FontError::new(FontErrorKind::InvalidFont))?
    }

    fn read(bytes: &[u8], face_index: u16) -> Result<Self> {
        let font =
            Font::load(bytes, usize::from(face_index), Options::default()).map_err(upstream)?;
        required(&font, "head")?.range(0, 54, 1)?;
        required(&font, "hhea")?.range(0, 36, 1)?;
        let maxp = required(&font, "maxp")?;
        let version = maxp.u32(0)?;
        maxp.range(0, if version == 0x0001_0000 { 32 } else { 6 }, 1)?;
        if version != 0x0001_0000 && version != 0x0000_5000 {
            return Err(maxp.err(0));
        }
        let head = font
            .head()
            .map_err(upstream)?
            .ok_or_else(|| missing("head"))?;
        let hhea = font
            .hhea()
            .map_err(upstream)?
            .ok_or_else(|| missing("hhea"))?;
        let maxp = font
            .maxp()
            .map_err(upstream)?
            .ok_or_else(|| missing("maxp"))?;
        if head.version_raw != 0x0001_0000
            || head.magic_number != 0x5f0f_3cf5
            || !(16..=16384).contains(&head.units_per_em)
        {
            return Err(FontError::malformed("head", 0));
        }
        if hhea.version_raw != 0x0001_0000 || hhea.metric_data_format != 0 {
            return Err(FontError::malformed("hhea", 0));
        }
        let n = usize::from(maxp.num_glyphs);
        let m = usize::from(hhea.number_of_h_metrics);
        if n == 0 || m == 0 || m > n {
            return Err(FontError::malformed("hhea", 34));
        }
        let hmtx = required(&font, "hmtx")?;
        hmtx.range(0, m * 4 + (n - m) * 2, 1)?;
        let mut advances = Vec::with_capacity(n);
        for i in 0..n {
            advances.push(GlyphMetrics {
                advance: hmtx.u16(i.min(m - 1) * 4)?,
                left_side_bearing: hmtx.i16(if i < m {
                    i * 4 + 2
                } else {
                    m * 4 + (i - m) * 2
                })?,
            });
        }
        validate::cmap(required(&font, "cmap")?.bytes, maxp.num_glyphs)?;
        let mappings = font
            .cmap()
            .map_err(upstream)?
            .ok_or_else(|| missing("cmap"))?
            .mappings();
        if let Some(glyf) = font.table_data("glyf") {
            validate::loca(
                required(&font, "loca")?.bytes,
                glyf,
                head.index_to_loc_format,
                maxp.num_glyphs,
            )?;
        }
        let outlines = font
            .outlines()
            .map_err(upstream)?
            .ok_or_else(|| missing("glyf/CFF"))?;
        if outlines.count() != n {
            return Err(FontError::new(FontErrorKind::InvalidFont));
        }
        let bounds = bounds::read(&outlines)?;
        // Every glyph's points decode at load: a malformed glyph is an
        // InvalidFont instead of a mid-layout failure. CFF/CFF2 faces carry
        // no decoded points and refuse outlines at selection time.
        let points = if outlines.kind == OutlineKind::Glyf {
            let mut points = Vec::with_capacity(n);
            for i in 0..n {
                points.push(outlines.decode_points(i).map_err(upstream)?);
            }
            // A glyph claiming ink bounds without contours could never be
            // painted; the painter relies on "no contours, no ink".
            for (i, (bounds, outline)) in bounds.iter().flatten().zip(&points).enumerate() {
                if bounds.is_some() && outline.is_none() {
                    return Err(FontError::malformed("glyf", i));
                }
            }
            Some(points)
        } else {
            None
        };
        let math = MathTable::parse(required(&font, "MATH")?.bytes, maxp.num_glyphs)?;
        Ok(Self {
            identity: FontIdentity {
                sha256: Sha256::digest(bytes).into(),
                face_index,
            },
            metrics: FontMetrics {
                units_per_em: head.units_per_em,
                ascent: hhea.ascent,
                descent: hhea.descent,
                line_gap: hhea.line_gap,
            },
            math,
            mappings,
            advances,
            bounds,
            outlines: points,
        })
    }

    pub fn glyph_id(&self, character: char) -> Option<u16> {
        self.mappings
            .binary_search_by_key(&(character as u32), |&(code, _)| code)
            .ok()
            .map(|i| self.mappings[i].1 as u16)
            .filter(|&glyph| glyph != 0)
    }

    pub fn glyph_metrics(&self, glyph: u16) -> Result<GlyphMetrics> {
        self.advances
            .get(usize::from(glyph))
            .copied()
            .ok_or_else(|| FontError::new(FontErrorKind::MissingGlyph))
    }

    /// Unhinted scaling to CSS pixels; percentage constants are exposed separately.
    pub fn scale(&self, design_units: f64, font_size_px: f64) -> Result<f64> {
        check_size(font_size_px)?;
        let value = design_units * (font_size_px / f64::from(self.metrics.units_per_em));
        if !design_units.is_finite() || !value.is_finite() {
            return Err(FontError::new(FontErrorKind::InvalidSize));
        }
        Ok(if value == 0.0 { 0.0 } else { value })
    }

    /// The glyph's decoded contour points in font units (y up, baseline at
    /// origin). `None` is a valid contourless glyph; CFF/CFF2 faces refuse.
    pub fn outline(&self, glyph: u16) -> Result<Option<&GlyphOutline>> {
        self.glyph_metrics(glyph)?;
        Ok(self
            .outlines
            .as_ref()
            .ok_or_else(|| FontError::new(FontErrorKind::OutlinesUnavailable))?
            .get(usize::from(glyph))
            .ok_or_else(|| FontError::new(FontErrorKind::MissingGlyph))?
            .as_ref())
    }
}

/// A glyph outline in CSS pixels, kept in font orientation (y up, baseline
/// at the pen origin); the painter applies the final y flip.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphOutlinePx {
    pub contours: Vec<ContourPx>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContourPx {
    pub points: Vec<PointPx>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointPx {
    pub x: f64,
    pub y: f64,
    pub on_curve: bool,
}

fn scale_outline(outline: &GlyphOutline, scale: f64) -> GlyphOutlinePx {
    let contour = |c: &fontisan::sfnt::Contour| ContourPx {
        points: c
            .points
            .iter()
            .map(|p| PointPx {
                x: f64::from(p.x) * scale,
                y: f64::from(p.y) * scale,
                on_curve: p.on_curve,
            })
            .collect(),
    };
    GlyphOutlinePx {
        contours: outline.contours.iter().map(contour).collect(),
    }
}

fn upstream(_: fontisan::sfnt::Error) -> FontError {
    FontError::new(FontErrorKind::InvalidFont)
}
fn missing(tag: &'static str) -> FontError {
    FontError {
        kind: FontErrorKind::MissingTable,
        table: Some(tag),
        offset: None,
    }
}
fn required<'a>(font: &'a Font<'_>, tag: &'static str) -> Result<Reader<'a>> {
    font.table_data(tag)
        .map(|b| Reader::new(b, tag))
        .ok_or_else(|| missing(tag))
}
fn check_size(size: f64) -> Result<()> {
    if !size.is_finite() || size <= 0.0 || size > 4096.0 {
        return Err(FontError::new(FontErrorKind::InvalidSize));
    }
    Ok(())
}

/// Resolved glyph geometry in CSS pixels, with the selected per-call resource.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedGlyph {
    pub resource_id: String,
    pub identity: FontIdentity,
    pub glyph_id: u16,
    pub advance_px: f64,
    pub left_side_bearing_px: f64,
    pub italics_correction_px: f64,
    pub top_accent_px: f64,
    /// Contour points in CSS pixels, font orientation; `None` when the glyph
    /// has no contours or the face carries no decodable outlines.
    pub outline_px: Option<GlyphOutlinePx>,
}

/// A stretch selection retains its owning font and design-unit measurements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StretchShape {
    Variant(GlyphVariant),
    Assembly(GlyphAssembly),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStretch {
    pub resource_id: String,
    pub identity: FontIdentity,
    pub shape: StretchShape,
    pub min_connector_overlap: u16,
}

/// One provider per call. Every resource is validated before any glyph is selected.
#[derive(Debug)]
pub struct FontProvider {
    faces: Vec<(String, FontFace)>,
    used: Vec<usize>,
}

impl FontProvider {
    pub fn new(resources: &[FontResource<'_>], fallback_order: &[&str]) -> Result<Self> {
        if resources.is_empty() || resources.len() > MAX_FONTS {
            return Err(FontError::new(FontErrorKind::ResourceLimit));
        }
        if fallback_order.len() != resources.len() {
            return Err(FontError::new(FontErrorKind::InvalidResources));
        }
        let mut ids = BTreeSet::new();
        let mut total = 0usize;
        for r in resources {
            let valid = !r.id.is_empty()
                && r.id.len() <= 64
                && r.id.as_bytes()[0].is_ascii_alphabetic()
                && r.id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-');
            if !valid || !ids.insert(r.id) {
                return Err(FontError::new(FontErrorKind::InvalidResources));
            }
            total = total
                .checked_add(r.bytes.len())
                .ok_or_else(|| FontError::new(FontErrorKind::ResourceLimit))?;
            if total > MAX_FONT_BYTES {
                return Err(FontError::new(FontErrorKind::ResourceLimit));
            }
            if r.sha256.len() != 64
                || !r
                    .sha256
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            {
                return Err(FontError::new(FontErrorKind::InvalidResources));
            }
            let digest = Sha256::digest(r.bytes);
            for (i, byte) in digest.iter().enumerate() {
                let expected = u8::from_str_radix(&r.sha256[i * 2..i * 2 + 2], 16)
                    .map_err(|_| FontError::new(FontErrorKind::InvalidResources))?;
                if *byte != expected {
                    return Err(FontError::new(FontErrorKind::HashMismatch));
                }
            }
        }
        let mut order = BTreeSet::new();
        let mut faces = Vec::with_capacity(resources.len());
        for &id in fallback_order {
            if !ids.contains(id) || !order.insert(id) {
                return Err(FontError::new(FontErrorKind::InvalidResources));
            }
            let resource = resources
                .iter()
                .find(|r| r.id == id)
                .ok_or_else(|| FontError::new(FontErrorKind::InvalidResources))?;
            faces.push((
                id.to_owned(),
                FontFace::load(resource.bytes, resource.face_index)?,
            ));
        }
        Ok(Self {
            faces,
            used: Vec::new(),
        })
    }

    /// Accessing primary metrics or constants records that resource as used.
    pub fn primary(&mut self) -> &FontFace {
        self.mark_used(0);
        &self.faces[0].1
    }

    pub fn math_constant_px(&mut self, constant: MathConstant, font_size_px: f64) -> Result<f64> {
        check_size(font_size_px)?;
        let font = self.primary();
        font.scale(f64::from(font.math.constants.value(constant)), font_size_px)
    }

    pub fn resolve(&mut self, character: char, font_size_px: f64) -> Result<ResolvedGlyph> {
        check_size(font_size_px)?;
        for (i, (id, font)) in self.faces.iter().enumerate() {
            if let Some(glyph_id) = font.glyph_id(character) {
                let metrics = font.glyph_metrics(glyph_id)?;
                // A CFF face resolves metrics without decoded points; only a
                // corrupt glyph (or a missing one) refuses the resolution.
                let outline_px = match font.outline(glyph_id) {
                    Ok(outline) => outline.map(|outline| {
                        scale_outline(outline, font_size_px / f64::from(font.metrics.units_per_em))
                    }),
                    Err(e) if e.kind == FontErrorKind::OutlinesUnavailable => None,
                    Err(e) => return Err(e),
                };
                let result = ResolvedGlyph {
                    resource_id: id.clone(),
                    identity: font.identity.clone(),
                    glyph_id,
                    advance_px: font.scale(f64::from(metrics.advance), font_size_px)?,
                    left_side_bearing_px: font
                        .scale(f64::from(metrics.left_side_bearing), font_size_px)?,
                    italics_correction_px: font.scale(
                        f64::from(font.math.italics_correction(glyph_id)),
                        font_size_px,
                    )?,
                    top_accent_px: font.scale(
                        font.math
                            .top_accent(glyph_id)
                            .map(f64::from)
                            .unwrap_or(f64::from(metrics.advance) / 2.0),
                        font_size_px,
                    )?,
                    outline_px,
                };
                self.mark_used(i);
                return Ok(result);
            }
        }
        Err(FontError::new(FontErrorKind::MissingGlyph))
    }

    /// Select the first font with a sufficient variant or an assembly description.
    /// Expansion and positioning of assembly parts belong to layout.
    pub fn resolve_stretch(
        &mut self,
        character: char,
        direction: Direction,
        minimum_px: f64,
        font_size_px: f64,
    ) -> Result<ResolvedStretch> {
        check_size(font_size_px)?;
        if !minimum_px.is_finite() || minimum_px < 0.0 {
            return Err(FontError::new(FontErrorKind::InvalidSize));
        }
        for (i, (id, font)) in self.faces.iter().enumerate() {
            let Some(glyph) = font.glyph_id(character) else {
                continue;
            };
            let Some(construction) = font.math.construction(glyph, direction) else {
                continue;
            };
            let scale = font_size_px / f64::from(font.metrics.units_per_em);
            let shape = if let Some(&variant) = construction
                .variants
                .iter()
                .find(|v| f64::from(v.advance) * scale >= minimum_px)
            {
                StretchShape::Variant(variant)
            } else if let Some(assembly) = &construction.assembly {
                StretchShape::Assembly(assembly.clone())
            } else {
                continue;
            };
            let result = ResolvedStretch {
                resource_id: id.clone(),
                identity: font.identity.clone(),
                shape,
                min_connector_overlap: font.math.min_connector_overlap,
            };
            self.mark_used(i);
            return Ok(result);
        }
        Err(FontError::new(FontErrorKind::MissingGlyph))
    }

    /// Actually selected file/face identities, deduplicated in first-use order.
    pub fn fonts_used(&self) -> Vec<FontIdentity> {
        self.used
            .iter()
            .map(|&i| self.faces[i].1.identity.clone())
            .collect()
    }

    fn mark_used(&mut self, index: usize) {
        let identity = &self.faces[index].1.identity;
        if !self
            .used
            .iter()
            .any(|&i| self.faces[i].1.identity == *identity)
        {
            self.used.push(index);
        }
    }
}
