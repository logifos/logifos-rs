//! Small rules for the operators in the accepted Formula wire contract.
//! MathML Core 3.2.4.2 and appendix B.1 define form, category and em spacing.

use super::LayoutError;

/// The vertical fence delimiters: stretchy along the block axis with no
/// spacing on either side (Core's operator dictionary, category F).
pub(super) const FENCES: &[char] = &[
    '(', ')', '[', ']', '{', '}', '|', '\u{2016}', '\u{2308}', '\u{2309}', '\u{230a}', '\u{230b}',
    '\u{27e8}', '\u{27e9}',
];

pub(super) fn is_fence(text: &str) -> bool {
    let mut chars = text.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if FENCES.contains(&c))
}

pub(super) fn validate(text: &str) -> Result<(), LayoutError> {
    if text.len() > 16 {
        return Err(LayoutError::ResourceLimit);
    }
    match text {
        "+" | "-" | "=" | "*" | "×" => Ok(()),
        _ if is_fence(text) => Ok(()),
        _ => Err(LayoutError::UnsupportedOperator(text.to_owned())),
    }
}

pub(super) fn space(text: &str, first: bool, multiple: bool, size: f64) -> f64 {
    // Prefix addition/subtraction are category D. At other positions the
    // dictionary fallback to infix supplies B; multiplication supplies C.
    // Equality is category A. Fences are category F: no spacing on either
    // side. No five-operator postfix entry exists.
    let units = match text {
        "+" | "-" if first && multiple => 0.0,
        "+" | "-" => 4.0,
        "*" | "×" => 3.0,
        "=" => 5.0,
        _ if is_fence(text) => 0.0,
        _ => unreachable!("operator validated before spacing"),
    };
    size * (units / 18.0)
}
