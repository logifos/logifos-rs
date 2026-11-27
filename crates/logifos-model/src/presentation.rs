//! Typed presentation hints, independent of XML spellings.

use crate::Length;

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
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct PresentationStyle {
    pub display: Option<MathDisplay>,
    pub displaystyle: Option<bool>,
    pub script_level: Option<ScriptLevel>,
    pub math_variant: Option<MathVariant>,
    pub math_color: Option<MathColor>,
    pub right_space: Option<MathSpace>,
    pub accent: Option<bool>,
    pub column_align: Option<ColumnAlign>,
    /// The mstyle floor the script-size scaling never drops below.
    pub script_min_size: Option<Length>,
    /// The mo stretch ceiling (Core 3.2.4): a percentage of the natural
    /// extent or an absolute length.
    pub max_size: Option<Length>,
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

    /// The canonical MathML spelling; the plurimath FontStyle subclasses
    /// carry it as their second parameter.
    pub fn spelling(self) -> &'static str {
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
    /// A `#rrggbb` spelling, resolved at paint time.
    Hex(u32),
}

impl MathColor {
    /// The MathML spelling plurimath's Color wrapper carries as its Text.
    pub fn spelling(self) -> String {
        match self {
            Self::Red => "red".into(),
            Self::Blue => "blue".into(),
            Self::Hex(value) => format!("#{value:06x}"),
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "red" => Some(Self::Red),
            "blue" => Some(Self::Blue),
            hex => {
                let digits = hex.strip_prefix('#')?;
                if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return None;
                }
                Some(Self::Hex(
                    u32::from_str_radix(&digits.to_ascii_lowercase(), 16).ok()?,
                ))
            }
        }
    }
}

/// MathML spaces, measured in eighteenths of an em: the named spaces plus
/// an authored em length (Core deprecates the names in favor of lengths;
/// both carry here).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MathSpace {
    VeryVeryThin,
    VeryThin,
    Thin,
    Medium,
    Thick,
    VeryThick,
    VeryVeryThick,
    /// An authored em length; parse rejects non-finite values, so equality
    /// stays reflexive.
    Em(f64),
}

impl Eq for MathSpace {}

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
            _ => {
                let text = value.trim();
                let number = text
                    .strip_suffix("em")
                    .unwrap_or(text)
                    .parse::<f64>()
                    .ok()?;
                if !number.is_finite() {
                    return None;
                }
                Self::Em(number)
            }
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
            Self::Em(em) => em * 18.0,
        }
    }

    /// The CSS value the writer serializes; lengths render as their em
    /// spelling.
    pub fn to_css(self) -> String {
        match self {
            Self::Em(em) => format!("{em}em"),
            Self::VeryVeryThin => "veryverythinmathspace".to_owned(),
            Self::VeryThin => "verythinmathspace".to_owned(),
            Self::Thin => "thinmathspace".to_owned(),
            Self::Medium => "mediummathspace".to_owned(),
            Self::Thick => "thickmathspace".to_owned(),
            Self::VeryThick => "verythickmathspace".to_owned(),
            Self::VeryVeryThick => "veryverythickmathspace".to_owned(),
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
        let variant_symbol = self.math_variant.is_some() && matches!(content, Formula::Symbol(_));
        self != Self::default()
            && !matches!(content, Formula::Styled { .. })
            && (grouping
                || variant_symbol
                || matches!(
                    content,
                    Formula::Operator(_) | Formula::Over(..) | Formula::UnderOver(..)
                ))
            && (self.display.is_none() || (root && matches!(content, Formula::Sequence(_))))
            && ((self.displaystyle.is_none()
                && self.script_level.is_none()
                && self.math_variant.is_none()
                && self.math_color.is_none())
                || (grouping && !cell)
                || (variant_symbol
                    && self.display.is_none()
                    && self.displaystyle.is_none()
                    && self.script_level.is_none()
                    && self.math_color.is_none()
                    && self.accent.is_none()
                    && !cell))
            && (self.right_space.is_none() || matches!(content, Formula::Operator(_)))
            && (self.accent.is_none()
                || matches!(content, Formula::Over(..) | Formula::UnderOver(..)))
            && (self.column_align.is_none() || (cell && matches!(content, Formula::Sequence(_))))
    }
}
