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

/// Explicit hints on a row-like MathML element, inherited by its children.
/// Missing hints inherit the caller's style. `display` is valid only on math.
/// Other element kinds continue to refuse these attributes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PresentationStyle {
    pub display: Option<MathDisplay>,
    pub displaystyle: Option<bool>,
    pub script_level: Option<ScriptLevel>,
}
