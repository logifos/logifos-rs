//! Validate every attribute before traversing a presentation subtree.

use logifos_model::{
    ColumnAlign, MathColor, MathDisplay, MathSpace, MathVariant, PresentationStyle, ScriptLevel,
};

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
        // Namespace declarations identify names and semantic-annotation
        // attributes carry no presentation data; both drop here.
        if attribute.name.local == "xmlns"
            || attribute.name.prefix.as_deref() == Some("xmlns")
            || matches!(attribute.name.local.as_str(), "xref" | "intent")
        {
            continue;
        }
        let valid = attribute.name.namespace.is_none()
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
                "displaystyle" if grouping => {
                    style.displaystyle = if value.eq_ignore_ascii_case("true") {
                        Some(true)
                    } else if value.eq_ignore_ascii_case("false") {
                        Some(false)
                    } else {
                        None
                    };
                    style.displaystyle.is_some()
                }
                "scriptlevel" if grouping => {
                    style.script_level = script_level(value);
                    style.script_level.is_some()
                }
                // A token's mathvariant styles the token itself.
                "mathvariant" if grouping || is_token(&element.name.local) => {
                    style.math_variant = MathVariant::parse(value);
                    style.math_variant.is_some()
                }
                "mathcolor" if grouping => {
                    style.math_color = match value.as_str() {
                        "red" => Some(MathColor::Red),
                        "blue" => Some(MathColor::Blue),
                        _ => None,
                    };
                    style.math_color.is_some()
                }
                "rspace" if element.name.local == "mo" => {
                    style.right_space = MathSpace::parse(value);
                    style.right_space.is_some()
                }
                "accent" if matches!(element.name.local.as_str(), "mover" | "munderover") => {
                    style.accent = match value.as_str() {
                        "true" => Some(true),
                        "false" => Some(false),
                        _ => None,
                    };
                    style.accent.is_some()
                }
                "columnalign" if element.name.local == "mtd" => {
                    style.column_align = (value == "center").then_some(ColumnAlign::Center);
                    style.column_align.is_some()
                }
                // mspace's extents are content, consumed by the mspace arm
                // (Core 3.2.5); presentation just admits them here.
                "width" | "height" | "depth" if element.name.local == "mspace" => true,
                // A layout-owned hint: the model keeps the fraction's
                // structure, the thickness stays with the layout batch.
                "linethickness" if element.name.local == "mfrac" => true,
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

/// The MathML token elements: identifiers, numbers, operators and text.
fn is_token(name: &str) -> bool {
    matches!(name, "mi" | "mn" | "mo" | "mtext")
}
