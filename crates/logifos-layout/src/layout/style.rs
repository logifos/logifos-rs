use super::LayoutError;
use crate::font::FontProvider;

/// Uniform options for this call; Formula has no presentation fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutOptions {
    /// CSS pixel size at script level zero, in (0, 4096].
    pub font_size_px: f64,
    /// Absolute nonnegative level relative to the supplied level-zero size.
    pub script_level: u8,
    /// MathML display style (display="block"); selects the display variants
    /// of the MATH table's fraction, script and radical constants.
    pub display: bool,
}

/// Effective size and flags for one construct, raised by scripted children.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedStyle {
    pub font_size_px: f64,
    /// The call's level-zero size; raising rescales from it, never from the
    /// current effective size, so equal levels resolve to equal sizes.
    pub base_font_size_px: f64,
    pub script_level: u8,
    pub display: bool,
    /// MathML math-shift compact; selects the cramped superscript constant.
    pub cramped: bool,
}

impl LayoutOptions {
    /// MathML Core section 4.5, from depth zero to the requested depth.
    /// OpenType MATH percentages apply at levels one and two. Each further
    /// level uses Core's factor 0.71; level two is relative to level zero.
    pub fn resolve(self, fonts: &mut FontProvider) -> Result<ResolvedStyle, LayoutError> {
        if !self.font_size_px.is_finite() || self.font_size_px <= 0.0 || self.font_size_px > 4096.0
        {
            return Err(LayoutError::InvalidOptions);
        }
        let font_size_px = self.font_size_px * schedule(fonts, self.script_level)?;
        if font_size_px <= 0.0 || !font_size_px.is_finite() {
            return Err(LayoutError::InvalidOptions);
        }
        Ok(ResolvedStyle {
            font_size_px,
            base_font_size_px: self.font_size_px,
            script_level: self.script_level,
            display: self.display,
            cramped: false,
        })
    }
}

impl ResolvedStyle {
    /// MathML Core section 3.4.4: a scripted child gains its depth's levels
    /// and compact displaystyle. `cramped` sets math-shift compact, which
    /// msub applies to its subscript; other scripts inherit the caller's
    /// shift.
    pub fn raise(
        &self,
        fonts: &mut FontProvider,
        levels: u8,
        cramped: bool,
    ) -> Result<Self, LayoutError> {
        let script_level = self
            .script_level
            .checked_add(levels)
            .ok_or(LayoutError::InvalidOptions)?;
        let font_size_px = self.base_font_size_px * schedule(fonts, script_level)?;
        if font_size_px <= 0.0 || !font_size_px.is_finite() {
            return Err(LayoutError::InvalidOptions);
        }
        Ok(Self {
            font_size_px,
            base_font_size_px: self.base_font_size_px,
            script_level,
            display: false,
            cramped,
        })
    }
}

/// The level's size as a factor of the level-zero size.
fn schedule(fonts: &mut FontProvider, level: u8) -> Result<f64, LayoutError> {
    if level == 0 {
        return Ok(1.0);
    }
    let c = &fonts.primary().math.constants;
    let first = c.script_percent_scale_down;
    if !(1..=100).contains(&first) {
        return Err(LayoutError::InvalidMathConstants);
    }
    let mut factor = f64::from(first) / 100.0;
    if level > 1 {
        let second = c.script_script_percent_scale_down;
        if !(1..=first).contains(&second) {
            return Err(LayoutError::InvalidMathConstants);
        }
        factor = f64::from(second) / 100.0;
        for _ in 2..level {
            factor *= 0.71;
        }
    }
    Ok(factor)
}
