use super::LayoutError;
use crate::font::FontProvider;
use logifos_model::{MathDisplay, PresentationStyle, ScriptLevel};

/// Inherited options for this call, before explicit Formula hints.
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
    /// The call's level-zero size, retained for callers inspecting the style.
    /// Explicit depth changes scale from the inherited size per Core 4.5.
    pub base_font_size_px: f64,
    pub script_level: i16,
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
            script_level: i16::from(self.script_level),
            display: self.display,
            cramped: false,
        })
    }
}

impl ResolvedStyle {
    pub(super) fn presentation(
        self,
        hints: PresentationStyle,
        fonts: &mut FontProvider,
    ) -> Result<Self, LayoutError> {
        let mut style = self;
        if let Some(display) = hints.display {
            style.display = display == MathDisplay::Block;
        }
        if let Some(displaystyle) = hints.displaystyle {
            style.display = displaystyle;
        }
        if let Some(level) = hints.script_level {
            let level = match level {
                ScriptLevel::Absolute(n) => i16::from(n),
                ScriptLevel::Add(n) => style.script_level + i16::from(n),
                ScriptLevel::Subtract(n) => style.script_level - i16::from(n),
            };
            style = style.at_level(fonts, level)?;
        }
        Ok(style)
    }

    fn at_level(self, fonts: &mut FontProvider, level: i16) -> Result<Self, LayoutError> {
        if !(-255..=255).contains(&level) {
            return Err(LayoutError::InvalidOptions);
        }
        let font_size_px = self.font_size_px * depth_scale(fonts, self.script_level, level)?;
        if !font_size_px.is_finite() || font_size_px <= 0.0 || font_size_px > 4096.0 {
            return Err(LayoutError::InvalidOptions);
        }
        Ok(Self {
            font_size_px,
            script_level: level,
            ..self
        })
    }

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
        let script_level = self.script_level + i16::from(levels);
        Ok(Self {
            display: false,
            cramped,
            ..self.at_level(fonts, script_level)?
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

/// Core 4.5 computes the scale between inherited and specified depths. Negative
/// depths grow the font; they are not clamped to zero or treated as absolutes.
fn depth_scale(fonts: &mut FontProvider, from: i16, to: i16) -> Result<f64, LayoutError> {
    if from == to {
        return Ok(1.0);
    }
    let (a, b) = (from.min(to), from.max(to));
    let mut exponent = b - a;
    let c = &fonts.primary().math.constants;
    let first = c.script_percent_scale_down;
    let second = c.script_script_percent_scale_down;
    if (a <= 1 && b >= 1 && !(1..=100).contains(&first))
        || (a <= 1 && b >= 2 && !(1..=first).contains(&second))
    {
        return Err(LayoutError::InvalidMathConstants);
    }
    let mut scale = 1.0;
    if a <= 0 && b >= 2 {
        scale *= f64::from(second) / 100.0;
        exponent -= 2;
    } else if a == 1 {
        scale *= f64::from(second) / f64::from(first);
        exponent -= 1;
    } else if b == 1 {
        scale *= f64::from(first) / 100.0;
        exponent -= 1;
    }
    scale *= 0.71_f64.powi(i32::from(exponent));
    Ok(if to < from { 1.0 / scale } else { scale })
}
