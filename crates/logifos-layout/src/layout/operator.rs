//! Small rules for the five operators in the accepted Formula wire contract.
//! MathML Core 3.2.4.2 and appendix B.1 define form, category and em spacing.

use super::LayoutError;

pub(super) fn validate(text: &str) -> Result<(), LayoutError> {
    if text.len() > 16 {
        return Err(LayoutError::ResourceLimit);
    }
    match text {
        "+" | "-" | "=" | "*" | "×" => Ok(()),
        _ => Err(LayoutError::UnsupportedOperator(text.to_owned())),
    }
}

pub(super) fn space(text: &str, first: bool, multiple: bool, size: f64) -> f64 {
    // Prefix addition/subtraction are category D. At other positions the
    // dictionary fallback to infix supplies B; multiplication supplies C.
    // Equality is category A. No five-operator postfix entry exists.
    let units = match text {
        "+" | "-" if first && multiple => 0.0,
        "+" | "-" => 4.0,
        "*" | "×" => 3.0,
        "=" => 5.0,
        _ => unreachable!("operator validated before spacing"),
    };
    size * (units / 18.0)
}
