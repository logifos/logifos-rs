//! Readers for the OpenType MATH table. Values remain in font design units.

use super::error::Result;
use super::reader::{Budget, Reader};
use std::collections::BTreeMap;

/// The MathValueRecord fields, in specification order (not the percentage fields).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum MathConstant {
    MathLeading,
    AxisHeight,
    AccentBaseHeight,
    FlattenedAccentBaseHeight,
    SubscriptShiftDown,
    SubscriptTopMax,
    SubscriptBaselineDropMin,
    SuperscriptShiftUp,
    SuperscriptShiftUpCramped,
    SuperscriptBottomMin,
    SuperscriptBaselineDropMax,
    SubSuperscriptGapMin,
    SuperscriptBottomMaxWithSubscript,
    SpaceAfterScript,
    UpperLimitGapMin,
    UpperLimitBaselineRiseMin,
    LowerLimitGapMin,
    LowerLimitBaselineDropMin,
    StackTopShiftUp,
    StackTopDisplayStyleShiftUp,
    StackBottomShiftDown,
    StackBottomDisplayStyleShiftDown,
    StackGapMin,
    StackDisplayStyleGapMin,
    StretchStackTopShiftUp,
    StretchStackBottomShiftDown,
    StretchStackGapAboveMin,
    StretchStackGapBelowMin,
    FractionNumeratorShiftUp,
    FractionNumeratorDisplayStyleShiftUp,
    FractionDenominatorShiftDown,
    FractionDenominatorDisplayStyleShiftDown,
    FractionNumeratorGapMin,
    FractionNumDisplayStyleGapMin,
    FractionRuleThickness,
    FractionDenominatorGapMin,
    FractionDenomDisplayStyleGapMin,
    SkewedFractionHorizontalGap,
    SkewedFractionVerticalGap,
    OverbarVerticalGap,
    OverbarRuleThickness,
    OverbarExtraAscender,
    UnderbarVerticalGap,
    UnderbarRuleThickness,
    UnderbarExtraDescender,
    RadicalVerticalGap,
    RadicalDisplayStyleVerticalGap,
    RadicalRuleThickness,
    RadicalExtraAscender,
    RadicalKernBeforeDegree,
    RadicalKernAfterDegree,
}

/// Font-wide math parameters. Percentages must not be scaled as design units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathConstants {
    pub script_percent_scale_down: i16,
    pub script_script_percent_scale_down: i16,
    pub delimited_sub_formula_min_height: u16,
    pub display_operator_min_height: u16,
    pub radical_degree_bottom_raise_percent: i16,
    values: [i16; 51],
}
impl MathConstants {
    pub fn value(&self, constant: MathConstant) -> i16 {
        self.values[constant as usize]
    }
}

/// Direction of glyph extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphVariant {
    pub glyph_id: u16,
    pub advance: u16,
}

/// A part's connectors and advance are measured in the extension direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssemblyPart {
    pub glyph_id: u16,
    pub start_connector: u16,
    pub end_connector: u16,
    pub full_advance: u16,
    pub extender: bool,
}

/// Parts retain specification order: bottom to top, or left to right.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphAssembly {
    pub italics_correction: i16,
    pub parts: Vec<AssemblyPart>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphConstruction {
    pub variants: Vec<GlyphVariant>,
    pub assembly: Option<GlyphAssembly>,
}

/// Validated MATH data for one face, with no host-font or process-global state.
#[derive(Debug, Clone)]
pub struct MathTable {
    pub constants: MathConstants,
    pub min_connector_overlap: u16,
    italics: BTreeMap<u16, i16>,
    accents: BTreeMap<u16, i16>,
    vertical: BTreeMap<u16, GlyphConstruction>,
    horizontal: BTreeMap<u16, GlyphConstruction>,
}

impl MathTable {
    /// Decode and validate all offsets/counts, including optional kerning data.
    /// Device corrections are validated but ignored for resolution-independent metrics.
    pub fn parse(bytes: &[u8], glyph_count: u16) -> Result<Self> {
        let r = Reader::new(bytes, "MATH");
        if r.u16(0)? != 1 {
            return Err(r.err(0));
        }
        r.u16(2)?; // Compatible minor revisions retain this header.
        let c = r.offset16(4, 10)?;
        c.range(0, 214, 1)?;
        let mut values = [0; 51];
        for (i, value) in values.iter_mut().enumerate() {
            *value = math_value(c, 8 + i * 4, 214)?;
        }
        let constants = MathConstants {
            script_percent_scale_down: c.i16(0)?,
            script_script_percent_scale_down: c.i16(2)?,
            delimited_sub_formula_min_height: c.u16(4)?,
            display_operator_min_height: c.u16(6)?,
            radical_degree_bottom_raise_percent: c.i16(212)?,
            values,
        };
        let mut budget = Budget::new();
        let info = r.offset16(6, 10)?;
        info.range(0, 8, 1)?;
        let italics = glyph_values(info.optional16(0, 8)?, glyph_count, &mut budget)?;
        let accents = glyph_values(info.optional16(2, 8)?, glyph_count, &mut budget)?;
        if let Some(cov) = info.optional16(4, 8)? {
            coverage(cov, glyph_count, &mut budget)?;
        }
        if let Some(kern) = info.optional16(6, 8)? {
            validate_kerns(kern, glyph_count, &mut budget)?;
        }
        let v = r.offset16(8, 10)?;
        let vc = usize::from(v.u16(6)?);
        let hc = usize::from(v.u16(8)?);
        let end = 10 + (vc + hc) * 2;
        v.range(10, vc + hc, 2)?;
        let vertical = constructions(v, 2, 10, vc, end, glyph_count, &mut budget)?;
        let horizontal = constructions(v, 4, 10 + vc * 2, hc, end, glyph_count, &mut budget)?;
        Ok(Self {
            constants,
            min_connector_overlap: v.u16(0)?,
            italics,
            accents,
            vertical,
            horizontal,
        })
    }

    pub fn italics_correction(&self, glyph: u16) -> i16 {
        self.italics.get(&glyph).copied().unwrap_or(0)
    }
    /// Missing entries use half the advance width at the provider boundary.
    pub fn top_accent(&self, glyph: u16) -> Option<i16> {
        self.accents.get(&glyph).copied()
    }
    pub fn construction(&self, glyph: u16, direction: Direction) -> Option<&GlyphConstruction> {
        match direction {
            Direction::Vertical => &self.vertical,
            Direction::Horizontal => &self.horizontal,
        }
        .get(&glyph)
    }
}

fn glyph(r: Reader<'_>, at: usize, count: u16) -> Result<u16> {
    let id = r.u16(at)?;
    if id >= count {
        return Err(r.err(at));
    }
    Ok(id)
}

fn coverage(r: Reader<'_>, glyph_count: u16, budget: &mut Budget) -> Result<Vec<u16>> {
    let count = usize::from(r.u16(2)?);
    let mut out = Vec::new();
    match r.u16(0)? {
        1 => {
            r.range(4, count, 2)?;
            budget.spend(count)?;
            for i in 0..count {
                let id = glyph(r, 4 + i * 2, glyph_count)?;
                if out.last().is_some_and(|&prev| prev >= id) {
                    return Err(r.err(4 + i * 2));
                }
                out.push(id);
            }
        }
        2 => {
            r.range(4, count, 6)?;
            for i in 0..count {
                let at = 4 + i * 6;
                let start = glyph(r, at, glyph_count)?;
                let end = glyph(r, at + 2, glyph_count)?;
                if start > end
                    || usize::from(r.u16(at + 4)?) != out.len()
                    || out.last().is_some_and(|&prev| prev >= start)
                {
                    return Err(r.err(at));
                }
                budget.spend(usize::from(end - start) + 1)?;
                out.extend(start..=end);
            }
        }
        _ => return Err(r.err(0)),
    }
    Ok(out)
}

fn covered(
    r: Reader<'_>,
    at: usize,
    end: usize,
    count: usize,
    glyphs: u16,
    budget: &mut Budget,
) -> Result<Vec<u16>> {
    let offset = r.u16(at)?;
    if offset == 0 && count == 0 {
        return Ok(Vec::new());
    }
    let ids = coverage(r.sub(usize::from(offset), end)?, glyphs, budget)?;
    if ids.len() != count {
        return Err(r.err(at));
    }
    Ok(ids)
}

fn math_value(r: Reader<'_>, at: usize, minimum: usize) -> Result<i16> {
    if let Some(d) = r.optional16(at + 2, minimum)? {
        let start = d.u16(0)?;
        let end = d.u16(2)?;
        let format = d.u16(4)?;
        if format != 0x8000 {
            if start > end || !(1..=3).contains(&format) {
                return Err(d.err(0));
            }
            let bits = 1usize << format;
            let words = ((usize::from(end - start) + 1) * bits).div_ceil(16);
            d.range(6, words, 2)?;
        }
    }
    r.i16(at)
}

fn glyph_values(
    r: Option<Reader<'_>>,
    glyphs: u16,
    budget: &mut Budget,
) -> Result<BTreeMap<u16, i16>> {
    let Some(r) = r else {
        return Ok(BTreeMap::new());
    };
    let count = usize::from(r.u16(2)?);
    r.range(4, count, 4)?;
    let end = 4 + count * 4;
    let ids = covered(r, 0, end, count, glyphs, budget)?;
    let mut out = BTreeMap::new();
    for (i, id) in ids.into_iter().enumerate() {
        out.insert(id, math_value(r, 4 + i * 4, end)?);
    }
    Ok(out)
}

fn validate_kerns(r: Reader<'_>, glyphs: u16, budget: &mut Budget) -> Result<()> {
    let count = usize::from(r.u16(2)?);
    let end = 4 + count * 8;
    r.range(4, count, 8)?;
    covered(r, 0, end, count, glyphs, budget)?;
    for i in 0..count * 4 {
        if let Some(k) = r.optional16(4 + i * 2, end)? {
            let heights = usize::from(k.u16(0)?);
            budget.spend(heights * 2 + 1)?;
            k.range(2, heights * 2 + 1, 4)?;
            let min = 2 + (heights * 2 + 1) * 4;
            let mut previous = None;
            for j in 0..heights * 2 + 1 {
                let value = math_value(k, 2 + j * 4, min)?;
                if j < heights {
                    if previous.is_some_and(|p| p >= value) {
                        return Err(k.err(2 + j * 4));
                    }
                    previous = Some(value);
                }
            }
        }
    }
    Ok(())
}

fn constructions(
    r: Reader<'_>,
    cov_at: usize,
    offsets: usize,
    count: usize,
    end: usize,
    glyphs: u16,
    budget: &mut Budget,
) -> Result<BTreeMap<u16, GlyphConstruction>> {
    let ids = covered(r, cov_at, end, count, glyphs, budget)?;
    let mut out = BTreeMap::new();
    for (i, id) in ids.into_iter().enumerate() {
        let c = r.offset16(offsets + i * 2, end)?;
        let n = usize::from(c.u16(2)?);
        c.range(4, n, 4)?;
        budget.spend(n)?;
        let mut variants = Vec::with_capacity(n);
        for j in 0..n {
            let advance = c.u16(6 + j * 4)?;
            if variants
                .last()
                .is_some_and(|v: &GlyphVariant| v.advance > advance)
            {
                return Err(c.err(6 + j * 4));
            }
            variants.push(GlyphVariant {
                glyph_id: glyph(c, 4 + j * 4, glyphs)?,
                advance,
            });
        }
        let assembly = c
            .optional16(0, 4 + n * 4)?
            .map(|a| assembly(a, glyphs, budget))
            .transpose()?;
        out.insert(id, GlyphConstruction { variants, assembly });
    }
    Ok(out)
}

fn assembly(r: Reader<'_>, glyphs: u16, budget: &mut Budget) -> Result<GlyphAssembly> {
    let n = usize::from(r.u16(4)?);
    if n == 0 {
        return Err(r.err(4));
    }
    r.range(6, n, 10)?;
    budget.spend(n)?;
    let italics_correction = math_value(r, 0, 6 + n * 10)?;
    let mut parts = Vec::with_capacity(n);
    for i in 0..n {
        let at = 6 + i * 10;
        let start_connector = r.u16(at + 2)?;
        let end_connector = r.u16(at + 4)?;
        let full_advance = r.u16(at + 6)?;
        let flags = r.u16(at + 8)?;
        if start_connector > full_advance || end_connector > full_advance || flags & !1 != 0 {
            return Err(r.err(at));
        }
        parts.push(AssemblyPart {
            glyph_id: glyph(r, at, glyphs)?,
            start_connector,
            end_connector,
            full_advance,
            extender: flags & 1 != 0,
        });
    }
    Ok(GlyphAssembly {
        italics_correction,
        parts,
    })
}
