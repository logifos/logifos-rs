//! Typed presentation hints, independent of XML spellings.

/// The root's display mode; standalone output has no surrounding line box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathDisplay {
    Block,
    Inline,
}

/// A bounded MathML math-depth hint. A sign means addition to the inherited
/// depth; an unsigned value resets it. Decreases can produce negative depths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptLevel {
    Absolute(u8),
    Add(u8),
    Subtract(u8),
}

/// Typed hints whose owning element is validated at each stage boundary.
/// Group hints inherit; spacing, accent and alignment belong to their operator,
/// over-script construct or table cell. `display` is valid only on math.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PresentationStyle {
    pub display: Option<MathDisplay>,
    pub displaystyle: Option<bool>,
    pub script_level: Option<ScriptLevel>,
    pub math_variant: Option<MathVariant>,
    pub math_color: Option<MathColor>,
    pub right_space: Option<MathSpace>,
    pub accent: Option<bool>,
    pub column_align: Option<ColumnAlign>,
}

/// A supported MathML mathvariant spelling, retained for font selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathVariant {
    Normal,
    Bold,
    Italic,
    BoldItalic,
    DoubleStruck,
    BoldFraktur,
    Script,
    BoldScript,
    Fraktur,
    SansSerif,
    BoldSansSerif,
    SansSerifItalic,
    SansSerifBoldItalic,
    Monospace,
}

impl MathVariant {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "normal" => Self::Normal,
            "bold" => Self::Bold,
            "italic" => Self::Italic,
            "bold-italic" => Self::BoldItalic,
            "double-struck" => Self::DoubleStruck,
            "bold-fraktur" => Self::BoldFraktur,
            "script" => Self::Script,
            "bold-script" => Self::BoldScript,
            "fraktur" => Self::Fraktur,
            "sans-serif" => Self::SansSerif,
            "bold-sans-serif" => Self::BoldSansSerif,
            "sans-serif-italic" => Self::SansSerifItalic,
            "sans-serif-bold-italic" => Self::SansSerifBoldItalic,
            "monospace" => Self::Monospace,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Bold => "bold",
            Self::Italic => "italic",
            Self::BoldItalic => "bold-italic",
            Self::DoubleStruck => "double-struck",
            Self::BoldFraktur => "bold-fraktur",
            Self::Script => "script",
            Self::BoldScript => "bold-script",
            Self::Fraktur => "fraktur",
            Self::SansSerif => "sans-serif",
            Self::BoldSansSerif => "bold-sans-serif",
            Self::SansSerifItalic => "sans-serif-italic",
            Self::SansSerifBoldItalic => "sans-serif-bold-italic",
            Self::Monospace => "monospace",
        }
    }
}

/// A deliberately bounded color vocabulary; other CSS colors still refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathColor {
    Red,
    Blue,
}

/// Named positive MathML spaces, measured in eighteenths of an em.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathSpace {
    VeryVeryThin,
    VeryThin,
    Thin,
    Medium,
    Thick,
    VeryThick,
    VeryVeryThick,
}

impl MathSpace {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "veryverythinmathspace" => Self::VeryVeryThin,
            "verythinmathspace" => Self::VeryThin,
            "thinmathspace" => Self::Thin,
            "mediummathspace" => Self::Medium,
            "thickmathspace" => Self::Thick,
            "verythickmathspace" => Self::VeryThick,
            "veryverythickmathspace" => Self::VeryVeryThick,
            _ => return None,
        })
    }

    /// The space in eighteenths of an em (MathML Core named spaces).
    pub fn units(self) -> f64 {
        match self {
            Self::VeryVeryThin => 1.0,
            Self::VeryThin => 2.0,
            Self::Thin => 3.0,
            Self::Medium => 4.0,
            Self::Thick => 5.0,
            Self::VeryThick => 6.0,
            Self::VeryVeryThick => 7.0,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::VeryVeryThin => "veryverythinmathspace",
            Self::VeryThin => "verythinmathspace",
            Self::Thin => "thinmathspace",
            Self::Medium => "mediummathspace",
            Self::Thick => "thickmathspace",
            Self::VeryThick => "verythickmathspace",
            Self::VeryVeryThick => "veryverythickmathspace",
        }
    }
}

/// Explicit centered cell alignment; other alignments remain unsupported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnAlign {
    Center,
}

impl PresentationStyle {
    /// Validates ownership even for directly constructed Formula values.
    /// Cell styles are only valid in an mtd context, never on an ordinary row.
    pub fn valid_for(self, content: &crate::Formula, root: bool, cell: bool) -> bool {
        use crate::Formula;
        let grouping = matches!(
            content,
            Formula::Sequence(_)
                | Formula::Style(_)
                | Formula::ErrorGroup(_)
                | Formula::Padded(_)
                | Formula::Phantom(_)
        );
        self != Self::default()
            && !matches!(content, Formula::Styled { .. })
            && (grouping
                || matches!(
                    content,
                    Formula::Operator(_) | Formula::Over(..) | Formula::UnderOver(..)
                ))
            && (self.display.is_none() || (root && matches!(content, Formula::Sequence(_))))
            && ((self.displaystyle.is_none()
                && self.script_level.is_none()
                && self.math_variant.is_none()
                && self.math_color.is_none())
                || (grouping && !cell))
            && (self.right_space.is_none() || matches!(content, Formula::Operator(_)))
            && (self.accent.is_none()
                || matches!(content, Formula::Over(..) | Formula::UnderOver(..)))
            && (self.column_align.is_none() || (cell && matches!(content, Formula::Sequence(_))))
    }
}
