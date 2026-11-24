//! Validate every attribute before traversing a presentation subtree.

use logifos_model::{MathDisplay, PresentationStyle, ScriptLevel};

use super::{refusal, Element, TransformError, TransformErrorKind};

pub(super) fn attributes(
    element: &Element,
    location: &str,
) -> Result<PresentationStyle, TransformError> {
    let mut style = PresentationStyle::default();
    let grouping = matches!(
        element.name.local.as_str(),
        "math" | "mrow" | "mstyle" | "merror" | "mpadded" | "mphantom"
    );
    for (index, attribute) in element.attributes.iter().enumerate() {
        let value = &attribute.value;
        let valid = grouping
            && attribute.name.namespace.is_none()
            && match attribute.name.local.as_str() {
                "display" if element.name.local == "math" => {
                    style.display = if value.eq_ignore_ascii_case("block") {
                        Some(MathDisplay::Block)
                    } else if value.eq_ignore_ascii_case("inline") {
                        Some(MathDisplay::Inline)
                    } else {
                        None
                    };
                    style.display.is_some()
                }
                "displaystyle" => {
                    style.displaystyle = if value.eq_ignore_ascii_case("true") {
                        Some(true)
                    } else if value.eq_ignore_ascii_case("false") {
                        Some(false)
                    } else {
                        None
                    };
                    style.displaystyle.is_some()
                }
                "scriptlevel" => {
                    style.script_level = script_level(value);
                    style.script_level.is_some()
                }
                _ => false,
            };
        if !valid {
            return Err(attribute_error(element, location, index));
        }
    }
    Ok(style)
}

fn attribute_error(element: &Element, location: &str, index: usize) -> TransformError {
    let mut error = refusal(
        TransformErrorKind::UnsupportedAttribute,
        element,
        &format!("{location}/attributes/{index}"),
    );
    error.attribute = Some(Box::new(element.attributes[index].name.clone()));
    error
}

fn script_level(value: &str) -> Option<ScriptLevel> {
    let (digits, sign) = match value.as_bytes().first() {
        Some(b'+') => (&value[1..], 1),
        Some(b'-') => (&value[1..], -1),
        _ => (value, 0),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let level = digits.parse::<u8>().ok()?;
    Some(match sign {
        1 => ScriptLevel::Add(level),
        -1 => ScriptLevel::Subtract(level),
        _ => ScriptLevel::Absolute(level),
    })
}
