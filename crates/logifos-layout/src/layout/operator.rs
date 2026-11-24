//! Small rules for the operators in the accepted Formula wire contract.
//! MathML Core 3.2.4.2 and appendix B.1 define form, category and em spacing.

use super::LayoutError;
use logifos_model::operator::{self, OperatorForm as Form};

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
    // Empty operators lay out as nothing; large ones as display variants.
    if !operator::is_operator_text(text) {
        return Err(LayoutError::UnsupportedOperator(text.to_owned()));
    }
    Ok(())
}

pub(super) fn large(text: &str) -> bool {
    [Form::Prefix, Form::Infix, Form::Postfix]
        .into_iter()
        .any(|form| operator::category(text, form).is_large())
}

/// The form of an operator from its place among the non-empty row children.
pub(super) fn form(index: usize, count: usize) -> Form {
    match (index, count) {
        (_, 0 | 1) => Form::Infix,
        (0, _) => Form::Prefix,
        (i, n) if i + 1 == n => Form::Postfix,
        _ => Form::Infix,
    }
}

/// The spacing in pixels; an explicit rspace replaces the trailing space.
pub(super) fn space(text: &str, form: Form, size: f64, right: Option<f64>) -> (f64, f64) {
    let (lead, trail) = dictionary_space(text, form, size);
    (lead, right.unwrap_or(trail))
}

fn dictionary_space(text: &str, form: Form, size: f64) -> (f64, f64) {
    // Core 3.2.4.2: the dictionary category supplies the leading space in
    // 1/18 em on each side. Fences are category F or G: no space on either side.
    if is_fence(text) {
        return (0.0, 0.0);
    }
    let (lead, trail) = operator::resolve(text, form).spacing_units();
    (size * (lead / 18.0), size * (trail / 18.0))
}
