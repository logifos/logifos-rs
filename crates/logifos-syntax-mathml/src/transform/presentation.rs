//! Validate every attribute before traversing a presentation subtree.

use logifos_model::{
    ColumnAlign, MathColor, MathDisplay, MathSpace, MathVariant, PresentationStyle, ScriptLevel,
};

use super::{parse_length, refusal, Element, TransformError, TransformErrorKind};

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
                    style.math_color = MathColor::parse(value);
                    style.math_color.is_some()
                }
                "rspace" if element.name.local == "mo" => {
                    // Plurimath stamps the rounded thick-space default
                    // (0.2778em = 5/18 em) on every operator it serializes;
                    // the stamp carries nothing, so it drops instead of
                    // refusing. Named spaces retain; any other length stays
                    // an explicit refusal until the spacing model carries
                    // dimensions.
                    if is_medium_space_stamp(value) {
                        true
                    } else {
                        style.right_space = MathSpace::parse(value);
                        style.right_space.is_some()
                    }
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
                // The script-size floor rides mstyle into layout (Core
                // 3.1.4); a length-percentage resolves against the size.
                "scriptminsize" if element.name.local == "mstyle" => {
                    style.script_min_size = parse_length(value);
                    style.script_min_size.is_some()
                }
                // The stretch ceiling rides mo into layout (Core 3.2.4).
                "maxsize" if element.name.local == "mo" => {
                    style.max_size = parse_length(value);
                    style.max_size.is_some()
                }
                // columnlines="none" is the table default and carries
                // nothing; "solid" rides the tree's TableLines. Any other
                // spelling (dashed, per-column lists) stays a refusal.
                "columnlines" if element.name.local == "mtable" => {
                    matches!(value.as_str(), "none" | "solid")
                }
                // mspace's extents are content, consumed by the mspace arm
                // (Core 3.2.5); presentation just admits them here.
                "width" | "height" | "depth" if element.name.local == "mspace" => true,
                // A layout-owned hint: the model keeps the fraction's
                // structure, the thickness stays with the layout batch.
                "linethickness" if element.name.local == "mfrac" => true,
                // bevelled flips the fraction's layout; both boolean
                // spellings are accepted and the mfrac arm consumes it.
                "bevelled" if element.name.local == "mfrac" => {
                    matches!(value.as_str(), "true" | "false")
                }
                // The elementary-stack geometry hints are consumed by the
                // mstack/msrow/msgroup arms (the model carries them).
                "stackalign" if element.name.local == "mstack" => true,
                "position"
                    if matches!(
                        element.name.local.as_str(),
                        "msrow" | "msgroup" | "mscarries"
                    ) =>
                {
                    true
                }
                "shift" if element.name.local == "msgroup" => true,
                "length" if element.name.local == "msline" => true,
                "crossout" if matches!(element.name.local.as_str(), "mscarries" | "mscarry") => {
                    true
                }
                "location" if matches!(element.name.local.as_str(), "mscarries" | "mscarry") => {
                    true
                }
                "longdivstyle" if element.name.local == "mlongdiv" => true,
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

/// Plurimath's operator rspace stamp: the thick space (5/18 em), emitted
/// rounded to four decimal places.
fn is_medium_space_stamp(value: &str) -> bool {
    value
        .trim()
        .strip_suffix("em")
        .and_then(|number| number.parse::<f64>().ok())
        .is_some_and(|em| (em - 5.0 / 18.0).abs() < 1e-4)
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
