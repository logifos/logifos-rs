//! Lossless conversion for the pinned upstream common-node subset.

use std::fmt;

use serde_json::{json, Map, Value};

use crate::{
    Formula, MathColor, MathDisplay, MathVariant, PresentationStyle, StackAlign, StackRow,
};

/// Immutable upstream definition identity; this is Rust source, not a JSON schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractPin {
    /// Upstream repository URL.
    pub origin: &'static str,
    /// Full upstream Git revision.
    pub revision: &'static str,
    /// Primary upstream common-node definition.
    pub definition_path: &'static str,
    /// SHA-256 of the exact primary definition bytes.
    pub sha256: &'static str,
}

/// Source authority for this boundary. See the crate's migration notes for the subset.
pub const FORMULA_CONTRACT: ContractPin = ContractPin {
    origin: "https://github.com/plurimath/plurimath-rs",
    revision: "74dbffe4c50e5ed33d25e0f29caddf7e8db776ad",
    definition_path: "plurimath-facade/src/model.rs",
    sha256: "6fc084ad002863b5910b556b8a18b3ebf666b147b8a4d700202460fa3402e7e1",
};

/// Maximum tree depth, counting the root as one.
pub const MAX_DEPTH: usize = 128;
/// Maximum number of nodes in a single conversion.
pub const MAX_NODES: usize = 100_000;

/// Stable categories for malformed or unrepresentable wire information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireErrorCode {
    /// A required field is absent.
    MissingField,
    /// A value has an incompatible JSON type or value.
    InvalidValue,
    /// A field has no lossless representation, including explicit null fields.
    UnsupportedField,
    /// A class has no lossless representation.
    UnsupportedClass,
    /// A typed operator has no supported upstream class identity.
    UnsupportedOperator,
    /// The tree exceeds a conversion budget.
    ResourceLimit,
}

/// A conversion refusal. No partial tree or wire value is returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireError {
    /// Machine-readable refusal category.
    pub code: WireErrorCode,
    /// RFC 6901 JSON Pointer; the empty string denotes the root.
    pub path: String,
}

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?} at {:?}", self.code, self.path)
    }
}

impl std::error::Error for WireError {}

fn refusal(code: WireErrorCode, path: &str) -> WireError {
    WireError {
        code,
        path: path.to_owned(),
    }
}

/// The plurimath FontStyle subclass suffix -> the variant it carries.
fn font_style_class(variant: MathVariant) -> &'static str {
    match variant {
        MathVariant::Normal => "Math::Function::FontStyle::Normal",
        MathVariant::Bold => "Math::Function::FontStyle::Bold",
        MathVariant::Italic => "Math::Function::FontStyle::Italic",
        MathVariant::BoldItalic => "Math::Function::FontStyle::BoldItalic",
        MathVariant::DoubleStruck => "Math::Function::FontStyle::DoubleStruck",
        MathVariant::BoldFraktur => "Math::Function::FontStyle::BoldFraktur",
        MathVariant::Script => "Math::Function::FontStyle::Script",
        MathVariant::BoldScript => "Math::Function::FontStyle::BoldScript",
        MathVariant::Fraktur => "Math::Function::FontStyle::Fraktur",
        MathVariant::SansSerif => "Math::Function::FontStyle::SansSerif",
        MathVariant::BoldSansSerif => "Math::Function::FontStyle::BoldSansSerif",
        MathVariant::SansSerifItalic => "Math::Function::FontStyle::SansSerifItalic",
        MathVariant::SansSerifBoldItalic => "Math::Function::FontStyle::SansSerifBoldItalic",
        MathVariant::Monospace => "Math::Function::FontStyle::Monospace",
    }
}

fn font_variant(suffix: &str) -> Option<MathVariant> {
    MathVariant::parse(match suffix {
        "Normal" => "normal",
        "Bold" => "bold",
        "Italic" => "italic",
        "BoldItalic" => "bold-italic",
        "DoubleStruck" => "double-struck",
        "BoldFraktur" => "bold-fraktur",
        "Script" => "script",
        "BoldScript" => "bold-script",
        "Fraktur" => "fraktur",
        "SansSerif" => "sans-serif",
        "BoldSansSerif" => "bold-sans-serif",
        "SansSerifItalic" => "sans-serif-italic",
        "SansSerifBoldItalic" => "sans-serif-bold-italic",
        "Monospace" => "monospace",
        _ => return None,
    })
}

/// Applies the under/over options hash: accent-only crosses as the accent
/// hint over the construct; any other option key refuses.
fn with_accent(fields: &Map<String, Value>, construct: Formula) -> Result<Formula, WireError> {
    match fields.get("options") {
        None => Ok(construct),
        Some(options) => {
            let object = options
                .as_object()
                .ok_or_else(|| refusal(WireErrorCode::InvalidValue, "/options"))?;
            closed(object, &["accent"], "/options")?;
            let accent = object
                .get("accent")
                .and_then(Value::as_bool)
                .ok_or_else(|| refusal(WireErrorCode::InvalidValue, "/options/accent"))?;
            Ok(Formula::Styled {
                style: PresentationStyle {
                    accent: Some(accent),
                    ..PresentationStyle::default()
                },
                content: Box::new(construct),
            })
        }
    }
}

/// Decodes plurimath's parallel sub/sup lists (null, a lone node, or an
/// array) into aligned script pairs; a hole or a length mismatch refuses.
fn script_pairs(
    subs: Value,
    sups: Value,
    path: &str,
    depth: usize,
    budget: &mut Budget,
) -> Result<Vec<crate::ScriptPair>, WireError> {
    let mut expand = |value: &Value, side: &str| -> Result<Vec<Option<Formula>>, WireError> {
        match value {
            Value::Null => Ok(Vec::new()),
            Value::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    out.push(Some(decode_value(
                        item,
                        &pointer(&pointer(path, side), &index.to_string()),
                        depth + 1,
                        budget,
                    )?));
                }
                Ok(out)
            }
            node => Ok(vec![Some(decode_value(
                node,
                &pointer(path, side),
                depth + 1,
                budget,
            )?)]),
        }
    };
    let subs = expand(&subs, "parameter_two")?;
    let sups = expand(&sups, "parameter_three")?;
    if subs.len() != sups.len() {
        return Err(refusal(WireErrorCode::UnsupportedField, path));
    }
    Ok(subs
        .into_iter()
        .zip(sups)
        .map(|(subscript, superscript)| crate::ScriptPair {
            subscript: subscript.map(Box::new),
            superscript: superscript.map(Box::new),
        })
        .collect())
}

/// The wrapped content of a styled group: a lone child stays itself,
/// several children become their sequence.
fn group_content(nodes: Vec<Formula>) -> Formula {
    match nodes.len() {
        1 => nodes.into_iter().next().expect("one child"),
        _ => Formula::Sequence(nodes),
    }
}

fn pointer(parent: &str, key: &str) -> String {
    format!("{parent}/{}", key.replace('~', "~0").replace('/', "~1"))
}

fn closed(object: &Map<String, Value>, allowed: &[&str], path: &str) -> Result<(), WireError> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(refusal(
                WireErrorCode::UnsupportedField,
                &pointer(path, key),
            ));
        }
    }
    Ok(())
}

fn required<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a Value, WireError> {
    object
        .get(key)
        .ok_or_else(|| refusal(WireErrorCode::MissingField, &pointer(path, key)))
}

#[derive(Default)]
struct Budget {
    nodes: usize,
}

impl Budget {
    fn enter(&mut self, depth: usize, path: &str) -> Result<(), WireError> {
        if depth > MAX_DEPTH || self.nodes >= MAX_NODES {
            return Err(refusal(WireErrorCode::ResourceLimit, path));
        }
        self.nodes += 1;
        Ok(())
    }
}

// Explicit identities keep named operators distinct from generic symbol text.
// The arithmetic spellings and the fence classes stay hand-written (their
// decode aliases are contract); every other spelling resolves through the
// generated plurimath symbol inventory.
fn operator_class(text: &str) -> Option<&'static str> {
    hand_operator_class(text)
        .or_else(|| lookup_sorted(crate::operator_wire::OPERATOR_CLASSES, text))
}

fn operator_text(class: &str) -> Option<&'static str> {
    hand_operator_text(class).or_else(|| lookup_sorted(crate::operator_wire::OPERATOR_TEXTS, class))
}

fn lookup_sorted(
    table: &'static [(&'static str, &'static str)],
    key: &str,
) -> Option<&'static str> {
    table
        .binary_search_by(|probe| probe.0.cmp(key))
        .ok()
        .map(|at| table[at].1)
}

fn hand_operator_class(text: &str) -> Option<&'static str> {
    match text {
        "+" => Some("Math::Symbols::Plus"),
        "-" => Some("Math::Symbols::Minus"),
        "=" => Some("Math::Symbols::Equal"),
        "*" => Some("Math::Symbols::Asterisk"),
        "×" => Some("Math::Symbols::Times"),
        "(" => Some("Math::Symbols::Paren::Lround"),
        ")" => Some("Math::Symbols::Paren::Rround"),
        "[" => Some("Math::Symbols::Paren::Lsquare"),
        "]" => Some("Math::Symbols::Paren::Rsquare"),
        "{" => Some("Math::Symbols::Paren::Lbrace"),
        "}" => Some("Math::Symbols::Paren::Rbrace"),
        "|" => Some("Math::Symbols::Paren::Norm"),
        "\u{2308}" => Some("Math::Symbols::Paren::Lceil"),
        "\u{2309}" => Some("Math::Symbols::Paren::Rceil"),
        "\u{230a}" => Some("Math::Symbols::Paren::Lfloor"),
        "\u{230b}" => Some("Math::Symbols::Paren::Rfloor"),
        "\u{27e8}" => Some("Math::Symbols::Paren::Langle"),
        // The deprecated confusable spellings decode to the mathematical
        // primaries (the Lbrack/Rbrack convention).
        "\u{2329}" => Some("Math::Symbols::Paren::Langle"),
        "\u{232a}" => Some("Math::Symbols::Paren::Rangle"),
        "\u{27e9}" => Some("Math::Symbols::Paren::Rangle"),
        // The word operators carry Function classes, not Symbols spellings.
        "mod" => Some("Math::Function::Mod"),
        _ => None,
    }
}

fn hand_operator_text(class: &str) -> Option<&'static str> {
    match class {
        "Math::Symbols::Plus" => Some("+"),
        "Math::Symbols::Minus" => Some("-"),
        "Math::Symbols::Equal" => Some("="),
        "Math::Symbols::Asterisk" => Some("*"),
        "Math::Symbols::Times" => Some("×"),
        "Math::Symbols::Paren::Lround" => Some("("),
        "Math::Symbols::Paren::Rround" => Some(")"),
        "Math::Symbols::Paren::Lsquare" | "Math::Symbols::Paren::Lbrack" => Some("["),
        "Math::Symbols::Paren::Rsquare" | "Math::Symbols::Paren::Rbrack" => Some("]"),
        "Math::Symbols::Paren::Lbrace" => Some("{"),
        "Math::Symbols::Paren::Rbrace" => Some("}"),
        "Math::Symbols::Paren::Norm" => Some("\u{2016}"),
        "Math::Symbols::Paren::Lceil" => Some("\u{2308}"),
        "Math::Symbols::Paren::Rceil" => Some("\u{2309}"),
        "Math::Symbols::Paren::Lfloor" => Some("\u{230a}"),
        "Math::Symbols::Paren::Rfloor" => Some("\u{230b}"),
        "Math::Symbols::Paren::Langle" => Some("\u{27e8}"),
        "Math::Symbols::Paren::Rangle" => Some("\u{27e9}"),
        "Math::Function::Mod" => Some("mod"),
        _ => None,
    }
}

impl Formula {
    /// Converts a closed upstream node, refusing all information the tree cannot carry.
    /// This replaces the bootstrap spellings and returns a typed error.
    pub fn from_wire(model: &Map<String, Value>) -> Result<Self, WireError> {
        decode_node(model, "", 1, &mut Budget::default())
    }

    /// Converts back to the same supported wire shape without adding defaults.
    /// Syntax-only under/over nodes, arbitrary operator strings and trees
    /// beyond the budgets are refused.
    pub fn to_wire(&self) -> Result<Value, WireError> {
        encode_node(self, "", 1, &mut Budget::default())
    }

    /// The children of a sequence; other variants have none.
    fn sequence(&self) -> Option<&[Formula]> {
        match self {
            Formula::Sequence(children) => Some(children),
            _ => None,
        }
    }
}

impl TryFrom<&Value> for Formula {
    type Error = WireError;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        decode_value(value, "", 1, &mut Budget::default())
    }
}

fn decode_value(
    value: &Value,
    path: &str,
    depth: usize,
    budget: &mut Budget,
) -> Result<Formula, WireError> {
    let model = value
        .as_object()
        .ok_or_else(|| refusal(WireErrorCode::InvalidValue, path))?;
    decode_node(model, path, depth, budget)
}

fn decode_node(
    model: &Map<String, Value>,
    path: &str,
    depth: usize,
    budget: &mut Budget,
) -> Result<Formula, WireError> {
    budget.enter(depth, path)?;
    closed(model, &["class", "fields"], path)?;
    let class = required(model, "class", path)?
        .as_str()
        .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &pointer(path, "class")))?;
    let fields_path = pointer(path, "fields");
    let fields = required(model, "fields", path)?
        .as_object()
        .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &fields_path))?;
    if class == "Math::Function::Color" {
        closed(fields, &["parameter_one", "parameter_two"], &fields_path)?;
        let name_node = required(fields, "parameter_one", &fields_path)?;
        let name_object = name_node.as_object().ok_or_else(|| {
            refusal(
                WireErrorCode::InvalidValue,
                &pointer(&fields_path, "parameter_one"),
            )
        })?;
        if name_object.get("class").and_then(Value::as_str) != Some("Math::Function::Text") {
            return Err(refusal(
                WireErrorCode::UnsupportedClass,
                &pointer(&fields_path, "parameter_one"),
            ));
        }
        let name = required(
            name_object
                .get("fields")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    refusal(
                        WireErrorCode::InvalidValue,
                        &pointer(&fields_path, "parameter_one"),
                    )
                })?,
            "parameter_one",
            &pointer(&fields_path, "parameter_one"),
        )?
        .as_str()
        .ok_or_else(|| {
            refusal(
                WireErrorCode::InvalidValue,
                &pointer(&pointer(&fields_path, "parameter_one"), "parameter_one"),
            )
        })?;
        let color = MathColor::parse(name).ok_or_else(|| {
            refusal(
                WireErrorCode::UnsupportedClass,
                &pointer(&pointer(&fields_path, "parameter_one"), "parameter_one"),
            )
        })?;
        let content = decode_value(
            required(fields, "parameter_two", &fields_path)?,
            &pointer(&fields_path, "parameter_two"),
            depth + 1,
            budget,
        )?;
        return Ok(Formula::Styled {
            style: PresentationStyle {
                math_color: Some(color),
                ..PresentationStyle::default()
            },
            content: Box::new(content),
        });
    }
    if let Some(suffix) = class.strip_prefix("Math::Function::FontStyle::") {
        let variant =
            font_variant(suffix).ok_or_else(|| refusal(WireErrorCode::UnsupportedClass, path))?;
        closed(fields, &["parameter_one", "parameter_two"], &fields_path)?;
        let spelling = required(fields, "parameter_two", &fields_path)?
            .as_str()
            .ok_or_else(|| {
                refusal(
                    WireErrorCode::InvalidValue,
                    &pointer(&fields_path, "parameter_two"),
                )
            })?;
        if spelling != variant.spelling() {
            return Err(refusal(
                WireErrorCode::InvalidValue,
                &pointer(&fields_path, "parameter_two"),
            ));
        }
        let content = decode_value(
            required(fields, "parameter_one", &fields_path)?,
            &pointer(&fields_path, "parameter_one"),
            depth + 1,
            budget,
        )?;
        return Ok(Formula::Styled {
            style: PresentationStyle {
                math_variant: Some(variant),
                ..PresentationStyle::default()
            },
            content: Box::new(content),
        });
    }
    match class {
        "Math::Formula" => {
            closed(fields, &["value", "displaystyle"], &fields_path)?;
            let value_path = pointer(&fields_path, "value");
            let children = required(fields, "value", &fields_path)?
                .as_array()
                .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &value_path))?;
            let nodes = children
                .iter()
                .enumerate()
                .map(|(index, child)| {
                    decode_value(
                        child,
                        &pointer(&value_path, &index.to_string()),
                        depth + 1,
                        budget,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            match fields.get("displaystyle") {
                None => Ok(Formula::Sequence(nodes)),
                Some(style) => {
                    let displaystyle = style.as_bool().ok_or_else(|| {
                        refusal(
                            WireErrorCode::InvalidValue,
                            &pointer(&fields_path, "displaystyle"),
                        )
                    })?;
                    let content = group_content(nodes);
                    Ok(Formula::Styled {
                        style: PresentationStyle {
                            displaystyle: Some(displaystyle),
                            ..PresentationStyle::default()
                        },
                        content: Box::new(content),
                    })
                }
            }
        }
        "Math::Symbols::Symbol" => {
            closed(fields, &["value"], &fields_path)?;
            let text = required(fields, "value", &fields_path)?
                .as_str()
                .ok_or_else(|| {
                    refusal(WireErrorCode::InvalidValue, &pointer(&fields_path, "value"))
                })?;
            Ok(Formula::Symbol(text.to_owned()))
        }
        "Math::Function::Frac" | "Math::Function::Power" | "Math::Function::Base" => {
            closed(fields, &["parameter_one", "parameter_two"], &fields_path)?;
            let first = decode_value(
                required(fields, "parameter_one", &fields_path)?,
                &pointer(&fields_path, "parameter_one"),
                depth + 1,
                budget,
            )?;
            let second = decode_value(
                required(fields, "parameter_two", &fields_path)?,
                &pointer(&fields_path, "parameter_two"),
                depth + 1,
                budget,
            )?;
            let pair = (Box::new(first), Box::new(second));
            Ok(match class {
                "Math::Function::Frac" => Formula::Fraction(pair.0, pair.1),
                "Math::Function::Power" => Formula::Superscript(pair.0, pair.1),
                _ => Formula::Subscript(pair.0, pair.1),
            })
        }
        "Math::Function::Sqrt" => {
            closed(fields, &["parameter_one"], &fields_path)?;
            let radicand = decode_value(
                required(fields, "parameter_one", &fields_path)?,
                &pointer(&fields_path, "parameter_one"),
                depth + 1,
                budget,
            )?;
            Ok(Formula::Sqrt(Box::new(radicand)))
        }
        "Math::Function::Root" => {
            closed(fields, &["parameter_one", "parameter_two"], &fields_path)?;
            let radicand = decode_value(
                required(fields, "parameter_one", &fields_path)?,
                &pointer(&fields_path, "parameter_one"),
                depth + 1,
                budget,
            )?;
            let index = decode_value(
                required(fields, "parameter_two", &fields_path)?,
                &pointer(&fields_path, "parameter_two"),
                depth + 1,
                budget,
            )?;
            Ok(Formula::Root(Box::new(radicand), Box::new(index)))
        }
        "Math::Formula::Mstyle" => {
            closed(fields, &["value"], &fields_path)?;
            let value_path = pointer(&fields_path, "value");
            let children = required(fields, "value", &fields_path)?
                .as_array()
                .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &value_path))?;
            let nodes = children
                .iter()
                .enumerate()
                .map(|(index, child)| {
                    decode_value(
                        child,
                        &pointer(&value_path, &index.to_string()),
                        depth + 1,
                        budget,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Formula::Style(nodes))
        }
        "Math::Function::Merror" | "Math::Function::Mpadded" | "Math::Function::Phantom" => {
            closed(fields, &["parameter_one"], &fields_path)?;
            let value_path = pointer(&fields_path, "parameter_one");
            let children = required(fields, "parameter_one", &fields_path)?
                .as_array()
                .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &value_path))?;
            let nodes = children
                .iter()
                .enumerate()
                .map(|(index, child)| {
                    decode_value(
                        child,
                        &pointer(&value_path, &index.to_string()),
                        depth + 1,
                        budget,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(match class {
                "Math::Function::Merror" => Formula::ErrorGroup(nodes),
                "Math::Function::Mpadded" => Formula::Padded(nodes),
                _ => Formula::Phantom(nodes),
            })
        }
        "Math::Function::Underset" | "Math::Function::Overset" => {
            closed(
                fields,
                &["parameter_one", "parameter_two", "options"],
                &fields_path,
            )?;
            let first = decode_value(
                required(fields, "parameter_one", &fields_path)?,
                &pointer(&fields_path, "parameter_one"),
                depth + 1,
                budget,
            )?;
            let second = decode_value(
                required(fields, "parameter_two", &fields_path)?,
                &pointer(&fields_path, "parameter_two"),
                depth + 1,
                budget,
            )?;
            let pair = (Box::new(first), Box::new(second));
            let construct = match class {
                "Math::Function::Underset" => Formula::Under(pair.0, pair.1),
                _ => Formula::Over(pair.0, pair.1),
            };
            with_accent(fields, construct)
        }
        "Math::Function::Underover" | "Math::Function::PowerBase" => {
            closed(
                fields,
                &[
                    "parameter_one",
                    "parameter_two",
                    "parameter_three",
                    "options",
                ],
                &fields_path,
            )?;
            let mut parameters = [None, None, None];
            for (index, name) in ["parameter_one", "parameter_two", "parameter_three"]
                .into_iter()
                .enumerate()
            {
                parameters[index] = Some(decode_value(
                    required(fields, name, &fields_path)?,
                    &pointer(&fields_path, name),
                    depth + 1,
                    budget,
                )?);
            }
            let [first, second, third] = parameters.map(|p| Box::new(p.expect("all decoded")));
            let construct = match class {
                "Math::Function::Underover" => Formula::UnderOver(first, second, third),
                _ => Formula::SubSup(first, second, third),
            };
            with_accent(fields, construct)
        }
        "Math::Function::Menclose" => {
            closed(fields, &["parameter_one", "parameter_two"], &fields_path)?;
            let notation = required(fields, "parameter_one", &fields_path)?
                .as_str()
                .ok_or_else(|| {
                    refusal(
                        WireErrorCode::InvalidValue,
                        &pointer(&fields_path, "parameter_one"),
                    )
                })?;
            let content = decode_value(
                required(fields, "parameter_two", &fields_path)?,
                &pointer(&fields_path, "parameter_two"),
                depth + 1,
                budget,
            )?;
            Ok(Formula::Menclose {
                notation: notation.to_owned().into_boxed_str(),
                content: Box::new(content),
            })
        }
        "Math::Function::Ms" => {
            closed(fields, &["parameter_one"], &fields_path)?;
            let text = required(fields, "parameter_one", &fields_path)?
                .as_str()
                .ok_or_else(|| {
                    refusal(
                        WireErrorCode::InvalidValue,
                        &pointer(&fields_path, "parameter_one"),
                    )
                })?;
            Ok(Formula::StringLiteral(text.to_owned()))
        }
        "Math::Function::Multiscript" => {
            closed(
                fields,
                &["parameter_one", "parameter_two", "parameter_three"],
                &fields_path,
            )?;
            let base_node = required(fields, "parameter_one", &fields_path)?;
            let base_object = base_node.as_object().ok_or_else(|| {
                refusal(
                    WireErrorCode::InvalidValue,
                    &pointer(&fields_path, "parameter_one"),
                )
            })?;
            let (base, postscripts) = if base_object.get("class").and_then(Value::as_str)
                == Some("Math::Function::PowerBase")
            {
                let nested = base_object
                    .get("fields")
                    .and_then(Value::as_object)
                    .ok_or_else(|| {
                        refusal(
                            WireErrorCode::InvalidValue,
                            &pointer(&fields_path, "parameter_one"),
                        )
                    })?;
                let base = decode_value(
                    required(
                        nested,
                        "parameter_one",
                        &pointer(&fields_path, "parameter_one"),
                    )?,
                    &pointer(&fields_path, "parameter_one/parameter_one"),
                    depth + 1,
                    budget,
                )?;
                let pairs_path = pointer(&fields_path, "parameter_one");
                let pairs = script_pairs(
                    required(
                        nested,
                        "parameter_two",
                        &pointer(&pairs_path, "parameter_two"),
                    )?
                    .clone(),
                    required(
                        nested,
                        "parameter_three",
                        &pointer(&pairs_path, "parameter_three"),
                    )?
                    .clone(),
                    &pairs_path,
                    depth + 1,
                    budget,
                )?;
                (base, pairs)
            } else {
                let base = decode_value(
                    base_node,
                    &pointer(&fields_path, "parameter_one"),
                    depth + 1,
                    budget,
                )?;
                (base, Vec::new())
            };
            let prescripts = match fields.get("parameter_two") {
                None | Some(Value::Null) => None,
                Some(subs) => {
                    let sups = required(fields, "parameter_three", &fields_path)?;
                    let pairs_path = pointer(&fields_path, "parameter_two");
                    let pairs =
                        script_pairs(subs.clone(), sups.clone(), &pairs_path, depth + 1, budget)?;
                    Some(pairs)
                }
            };
            Ok(Formula::MultiScripts {
                base: Box::new(base),
                postscripts,
                prescripts,
            })
        }
        "Math::Function::Table" => {
            closed(
                fields,
                &["value", "open_paren", "close_paren", "options"],
                &fields_path,
            )?;
            // Fence and styling fields carry no information when absent;
            // present ones name information the tree cannot carry yet.
            for name in ["open_paren", "close_paren"] {
                if fields.get(name).is_some_and(|value| !value.is_null()) {
                    return Err(refusal(
                        WireErrorCode::UnsupportedField,
                        &pointer(&fields_path, name),
                    ));
                }
            }
            if let Some(value) = fields.get("options") {
                if value.as_object().is_none_or(|options| !options.is_empty()) {
                    return Err(refusal(
                        WireErrorCode::UnsupportedField,
                        &pointer(&fields_path, "options"),
                    ));
                }
            }
            let value_path = pointer(&fields_path, "value");
            let rows = required(fields, "value", &fields_path)?
                .as_array()
                .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &value_path))?;
            let mut table = Vec::with_capacity(rows.len());
            for (row_index, row) in rows.iter().enumerate() {
                let row_path = pointer(&value_path, &row_index.to_string());
                let row = decode_tr(row, &row_path, depth + 1, budget)?;
                table.push(row);
            }
            Ok(Formula::Table(table))
        }
        "Math::Function::Stackrel" => {
            closed(fields, &["parameter_one", "parameter_two"], &fields_path)?;
            let rows = decode_stack_chain(fields, &fields_path, depth + 1, budget)?;
            Ok(Formula::Stack(StackAlign::DecimalPoint, rows))
        }
        _ => {
            let text = operator_text(class)
                .ok_or_else(|| refusal(WireErrorCode::UnsupportedClass, &pointer(path, "class")))?;
            closed(fields, &["value"], &fields_path)?;
            if !required(fields, "value", &fields_path)?.is_null() {
                return Err(refusal(
                    WireErrorCode::InvalidValue,
                    &pointer(&fields_path, "value"),
                ));
            }
            Ok(Formula::Operator(text.to_owned()))
        }
    }
}

/// Decodes one `Math::Function::Tr` row of cells into its cell formulas.
fn decode_tr(
    value: &Value,
    path: &str,
    depth: usize,
    budget: &mut Budget,
) -> Result<Vec<Formula>, WireError> {
    let row = value
        .as_object()
        .ok_or_else(|| refusal(WireErrorCode::InvalidValue, path))?;
    let class = required(row, "class", path)?
        .as_str()
        .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &pointer(path, "class")))?;
    if class != "Math::Function::Tr" {
        return Err(refusal(
            WireErrorCode::UnsupportedClass,
            &pointer(path, "class"),
        ));
    }
    let fields_path = pointer(path, "fields");
    let fields = required(row, "fields", path)?
        .as_object()
        .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &fields_path))?;
    closed(fields, &["parameter_one"], &fields_path)?;
    let cells_path = pointer(&fields_path, "parameter_one");
    let cells = required(fields, "parameter_one", &fields_path)?
        .as_array()
        .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &cells_path))?;
    let mut out = Vec::with_capacity(cells.len());
    for (index, cell) in cells.iter().enumerate() {
        let cell_path = pointer(&cells_path, &index.to_string());
        let cell = cell
            .as_object()
            .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &cell_path))?;
        let class = required(cell, "class", &cell_path)?
            .as_str()
            .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &pointer(&cell_path, "class")))?;
        if class != "Math::Function::Td" {
            return Err(refusal(
                WireErrorCode::UnsupportedClass,
                &pointer(&cell_path, "class"),
            ));
        }
        let fields_path = pointer(&cell_path, "fields");
        let fields = required(cell, "fields", &cell_path)?
            .as_object()
            .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &fields_path))?;
        closed(fields, &["parameter_one", "parameter_two"], &fields_path)?;
        if fields
            .get("parameter_two")
            .is_some_and(|value| !value.is_null())
        {
            return Err(refusal(
                WireErrorCode::UnsupportedField,
                &pointer(&fields_path, "parameter_two"),
            ));
        }
        let content_path = pointer(&fields_path, "parameter_one");
        let children = required(fields, "parameter_one", &fields_path)?
            .as_array()
            .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &content_path))?;
        let mut line = Vec::with_capacity(children.len());
        for (index, child) in children.iter().enumerate() {
            line.push(decode_value(
                child,
                &pointer(&content_path, &index.to_string()),
                depth + 1,
                budget,
            )?);
        }
        out.push(Formula::Sequence(line));
    }
    Ok(out)
}

fn encode_node(
    node: &Formula,
    path: &str,
    depth: usize,
    budget: &mut Budget,
) -> Result<Value, WireError> {
    budget.enter(depth, path)?;
    let fields_path = pointer(path, "fields");
    let (class, fields) = match node {
        Formula::Sequence(children) => {
            let value_path = pointer(&fields_path, "value");
            let values = children
                .iter()
                .enumerate()
                .map(|(index, child)| {
                    encode_node(
                        child,
                        &pointer(&value_path, &index.to_string()),
                        depth + 1,
                        budget,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            ("Math::Formula", json!({"value": values}))
        }
        // These syntax-side constructs have no mapping in the pinned subset.
        // Refuse rather than inventing a class or losing their placement.
        Formula::Space(..) => {
            return Err(refusal(WireErrorCode::UnsupportedClass, path));
        }
        Formula::Menclose { notation, content } => {
            let content = encode_node(
                content,
                &pointer(&fields_path, "parameter_two"),
                depth + 1,
                budget,
            )?;
            (
                "Math::Function::Menclose",
                json!({"parameter_one": notation, "parameter_two": content}),
            )
        }
        Formula::MultiScripts {
            base,
            postscripts,
            prescripts,
        } => {
            // Plurimath nests the postscripts into the base (a PowerBase
            // over the parallel sub/sup lists, single-element lists unwrapped
            // to the bare node) and carries the prescripts as the ternary's
            // parallel parameters. A pair with an empty slot has no aligned
            // plurimath representation and refuses.
            let mut flatten =
                |pairs: &[crate::ScriptPair], path: &str| -> Result<(Value, Value), WireError> {
                    let mut subs = Vec::with_capacity(pairs.len());
                    let mut sups = Vec::with_capacity(pairs.len());
                    for (index, pair) in pairs.iter().enumerate() {
                        let (sub, sup) = match (&pair.subscript, &pair.superscript) {
                            (Some(sub), Some(sup)) => (sub, sup),
                            _ => {
                                return Err(refusal(
                                    WireErrorCode::UnsupportedClass,
                                    &pointer(path, &index.to_string()),
                                ))
                            }
                        };
                        subs.push(encode_node(
                            sub,
                            &pointer(path, &format!("{index}/subscript")),
                            depth + 1,
                            budget,
                        )?);
                        sups.push(encode_node(
                            sup,
                            &pointer(path, &format!("{index}/superscript")),
                            depth + 1,
                            budget,
                        )?);
                    }
                    let unwrap_single = |mut items: Vec<Value>| {
                        if items.len() == 1 {
                            items.pop().expect("one item")
                        } else {
                            Value::Array(items)
                        }
                    };
                    Ok((unwrap_single(subs), unwrap_single(sups)))
                };
            let posts = if postscripts.is_empty() {
                None
            } else {
                Some(flatten(
                    postscripts,
                    &pointer(&fields_path, "parameter_one/postscripts"),
                )?)
            };
            let pres = match prescripts {
                None => None,
                Some(pairs) => Some(flatten(pairs, &pointer(&fields_path, "parameter_two"))?),
            };
            let parameter_one = match (&posts, postscripts.is_empty()) {
                (&None, true) => encode_node(
                    base,
                    &pointer(&fields_path, "parameter_one"),
                    depth + 1,
                    budget,
                )?,
                (Some((subs, sups)), _) => json!({
                    "class": "Math::Function::PowerBase",
                    "fields": {
                        "parameter_one": encode_node(
                            base,
                            &pointer(&fields_path, "parameter_one/base"),
                            depth + 1,
                            budget,
                        )?,
                        "parameter_two": subs,
                        "parameter_three": sups,
                    }
                }),
                _ => unreachable!("posts exist only when nonempty"),
            };
            let (parameter_two, parameter_three) = match pres {
                None => (Value::Null, Value::Null),
                Some((subs, sups)) => (subs, sups),
            };
            (
                "Math::Function::Multiscript",
                json!({
                    "parameter_one": parameter_one,
                    "parameter_two": parameter_two,
                    "parameter_three": parameter_three,
                }),
            )
        }
        Formula::Styled { style, content } => {
            // The hint mapping mirrors plurimath's own wrapping order: color
            // wraps the content innermost (a Color over a Text carrying the
            // name), the variant wraps that (its FontStyle class), and the
            // display hint crosses as the displaystyle. Accent belongs to
            // the under/over construct and rides the construct's options
            // hash. A hint pair this mapping cannot carry, and every other
            // presentation attribute, refuses rather than dropping.
            let carries = style.script_level.is_some()
                || style.right_space.is_some()
                || style.column_align.is_some();
            let derived = match style.display {
                Some(MathDisplay::Block) => Some(true),
                Some(MathDisplay::Inline) => Some(false),
                None => None,
            };
            let displaystyle = match (style.displaystyle, derived) {
                (Some(explicit), Some(from_display)) if explicit != from_display => {
                    return Err(refusal(WireErrorCode::UnsupportedClass, path));
                }
                (Some(explicit), _) => Some(explicit),
                (None, redundant) => redundant,
            };
            if carries {
                return Err(refusal(WireErrorCode::UnsupportedClass, path));
            }
            let content = encode_node(content, &pointer(&fields_path, "value"), depth + 1, budget)?;
            let content = match style.math_color {
                Some(color) => json!({
                    "class": "Math::Function::Color",
                    "fields": {
                        "parameter_one": {
                            "class": "Math::Function::Text",
                            "fields": {"parameter_one": color.spelling()},
                        },
                        "parameter_two": content,
                    }
                }),
                None => content,
            };
            if let Some(accent) = style.accent {
                if displaystyle.is_some() || style.math_variant.is_some() {
                    return Err(refusal(WireErrorCode::UnsupportedClass, path));
                }
                let mut node = content;
                let class = node
                    .get("class")
                    .and_then(Value::as_str)
                    .ok_or_else(|| refusal(WireErrorCode::UnsupportedClass, path))?;
                if !matches!(
                    class,
                    "Math::Function::Underset"
                        | "Math::Function::Overset"
                        | "Math::Function::Underover"
                ) {
                    return Err(refusal(WireErrorCode::UnsupportedClass, path));
                }
                node["fields"]["options"] = json!({"accent": accent});
                return Ok(node);
            }
            if let Some(variant) = style.math_variant {
                if displaystyle.is_some() {
                    return Err(refusal(WireErrorCode::UnsupportedClass, path));
                }
                (
                    font_style_class(variant),
                    json!({"parameter_one": content, "parameter_two": variant.spelling()}),
                )
            } else if let Some(displaystyle) = displaystyle {
                (
                    "Math::Formula",
                    json!({"value": [content], "displaystyle": displaystyle}),
                )
            } else if style.math_color.is_some() {
                // A bare Color wrapper is itself a node; return it directly.
                return Ok(content);
            } else {
                return Err(refusal(WireErrorCode::UnsupportedClass, path));
            }
        }
        Formula::Style(children) => {
            let value_path = pointer(&fields_path, "value");
            let mut encoded = Vec::with_capacity(children.len());
            for (index, child) in children.iter().enumerate() {
                encoded.push(encode_node(
                    child,
                    &pointer(&value_path, &index.to_string()),
                    depth + 1,
                    budget,
                )?);
            }
            ("Math::Formula::Mstyle", json!({"value": encoded}))
        }
        Formula::ErrorGroup(children) | Formula::Padded(children) | Formula::Phantom(children) => {
            let class = match node {
                Formula::ErrorGroup(_) => "Math::Function::Merror",
                Formula::Padded(_) => "Math::Function::Mpadded",
                _ => "Math::Function::Phantom",
            };
            let value_path = pointer(&fields_path, "parameter_one");
            let mut encoded = Vec::with_capacity(children.len());
            for (index, child) in children.iter().enumerate() {
                encoded.push(encode_node(
                    child,
                    &pointer(&value_path, &index.to_string()),
                    depth + 1,
                    budget,
                )?);
            }
            (class, json!({"parameter_one": encoded}))
        }
        Formula::Under(first, second) | Formula::Over(first, second) => {
            let class = match node {
                Formula::Under(..) => "Math::Function::Underset",
                _ => "Math::Function::Overset",
            };
            let first = encode_node(
                first,
                &pointer(&fields_path, "parameter_one"),
                depth + 1,
                budget,
            )?;
            let second = encode_node(
                second,
                &pointer(&fields_path, "parameter_two"),
                depth + 1,
                budget,
            )?;
            (
                class,
                json!({"parameter_one": first, "parameter_two": second}),
            )
        }
        Formula::UnderOver(first, second, third) | Formula::SubSup(first, second, third) => {
            let class = match node {
                Formula::UnderOver(..) => "Math::Function::Underover",
                _ => "Math::Function::PowerBase",
            };
            let first = encode_node(
                first,
                &pointer(&fields_path, "parameter_one"),
                depth + 1,
                budget,
            )?;
            let second = encode_node(
                second,
                &pointer(&fields_path, "parameter_two"),
                depth + 1,
                budget,
            )?;
            let third = encode_node(
                third,
                &pointer(&fields_path, "parameter_three"),
                depth + 1,
                budget,
            )?;
            (
                class,
                json!({"parameter_one": first, "parameter_two": second, "parameter_three": third}),
            )
        }
        Formula::StringLiteral(text) => ("Math::Function::Ms", json!({"parameter_one": text})),
        Formula::Symbol(text) => ("Math::Symbols::Symbol", json!({"value": text})),
        Formula::Operator(text) => {
            // A spelling with no Symbols class rides plurimath's generic
            // Symbols::Symbol carrying the text, exactly as plurimath's own
            // tree holds unknown operators.
            match operator_class(text) {
                Some(class) => (class, json!({"value": null})),
                None => ("Math::Symbols::Symbol", json!({"value": text})),
            }
        }
        Formula::Fraction(first, second)
        | Formula::Superscript(first, second)
        | Formula::Subscript(first, second)
        | Formula::Root(first, second) => {
            let class = match node {
                Formula::Fraction(..) => "Math::Function::Frac",
                Formula::Superscript(..) => "Math::Function::Power",
                Formula::Subscript(..) => "Math::Function::Base",
                _ => "Math::Function::Root",
            };
            let first = encode_node(
                first,
                &pointer(&fields_path, "parameter_one"),
                depth + 1,
                budget,
            )?;
            let second = encode_node(
                second,
                &pointer(&fields_path, "parameter_two"),
                depth + 1,
                budget,
            )?;
            (
                class,
                json!({"parameter_one": first, "parameter_two": second}),
            )
        }
        Formula::Sqrt(radicand) => {
            let first = encode_node(
                radicand,
                &pointer(&fields_path, "parameter_one"),
                depth + 1,
                budget,
            )?;
            ("Math::Function::Sqrt", json!({"parameter_one": first}))
        }
        Formula::Table(rows) => {
            let value_path = pointer(&fields_path, "value");
            let mut encoded = Vec::with_capacity(rows.len());
            for (index, row) in rows.iter().enumerate() {
                let row_path = pointer(&value_path, &index.to_string());
                let cells_path = pointer(&row_path, "fields/parameter_one");
                let mut cells = Vec::with_capacity(row.len());
                for (cell_index, cell) in row.iter().enumerate() {
                    let cell_path = pointer(&cells_path, &cell_index.to_string());
                    let content_path = pointer(&cell_path, "fields/parameter_one");
                    let content = cell
                        .sequence()
                        .ok_or_else(|| refusal(WireErrorCode::UnsupportedField, &content_path))?;
                    let mut children = Vec::with_capacity(content.len());
                    for (child_index, child) in content.iter().enumerate() {
                        children.push(encode_node(
                            child,
                            &pointer(&content_path, &child_index.to_string()),
                            depth + 1,
                            budget,
                        )?);
                    }
                    cells.push(json!({
                        "class": "Math::Function::Td",
                        "fields": {"parameter_one": children},
                    }));
                }
                encoded.push(json!({
                    "class": "Math::Function::Tr",
                    "fields": {"parameter_one": cells},
                }));
            }
            ("Math::Function::Table", json!({"value": encoded}))
        }
        Formula::Stack(align, rows) => {
            // plurimath's mstack is a Stackrel whose `parameter_two` rides
            // over `parameter_one`, so an N-row stack encodes as the nested
            // Stackrel chain that rendering implies. The upstream envelope
            // has no slot for `stackalign` or the row positions: a
            // non-default alignment or a positioned row refuses instead of
            // dropping.
            if *align != StackAlign::DecimalPoint {
                return Err(refusal(WireErrorCode::UnsupportedField, &fields_path));
            }
            (
                "Math::Function::Stackrel",
                encode_stack_chain(rows, &fields_path, depth, budget)?,
            )
        }
        // The carries and long-division vocabulary has no slot in the
        // pinned upstream envelope; refuse instead of inventing classes.
        Formula::LongDivision(_) => {
            return Err(refusal(WireErrorCode::UnsupportedClass, path));
        }
    };
    Ok(json!({"class": class, "fields": fields}))
}

/// The nested Stackrel chain for an elementary stack: each row rides
/// `parameter_two`, the rows under it ride `parameter_one`, and the empty
/// stack is a bare Stackrel.
fn encode_stack_chain(
    rows: &[StackRow],
    fields_path: &str,
    depth: usize,
    budget: &mut Budget,
) -> Result<Value, WireError> {
    let mut fields = json!({"parameter_two": Value::Null, "parameter_one": Value::Null});
    for (index, row) in rows.iter().enumerate().rev() {
        let row_path = pointer(fields_path, &format!("rows/{index}"));
        fields = json!({
            "parameter_two": encode_stack_row(row, &row_path, depth, budget)?,
            "parameter_one": json!({
                "class": "Math::Function::Stackrel",
                "fields": fields,
            }),
        });
    }
    Ok(fields)
}

/// One stack row on the wire: a plain formula sequence, an Msline, or a
/// Msgroup of rows.
fn encode_stack_row(
    row: &StackRow,
    row_path: &str,
    depth: usize,
    budget: &mut Budget,
) -> Result<Value, WireError> {
    let row_fields = pointer(row_path, "fields");
    match row {
        StackRow::Carries { .. } => Err(refusal(WireErrorCode::UnsupportedField, row_path)),
        StackRow::Cells { position, cells } => {
            if position.is_some() {
                return Err(refusal(WireErrorCode::UnsupportedField, row_path));
            }
            let value_path = pointer(&row_fields, "value");
            let children = cells
                .iter()
                .enumerate()
                .map(|(cell_index, cell)| {
                    encode_node(
                        cell,
                        &pointer(&value_path, &cell_index.to_string()),
                        depth + 1,
                        budget,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(json!({
                "class": "Math::Formula",
                "fields": {"value": children},
            }))
        }
        StackRow::Line { position, length } => {
            // The upstream envelope has no slot for the rule's position or
            // length; only the bare rule crosses.
            if position.is_some() || length.is_some() {
                return Err(refusal(WireErrorCode::UnsupportedField, row_path));
            }
            Ok(json!({
                "class": "Math::Function::Msline",
                "fields": {"parameter_one": Value::Null},
            }))
        }
        StackRow::Group {
            position,
            shift,
            rows: group,
        } => {
            if position.is_some() || shift.is_some() {
                return Err(refusal(WireErrorCode::UnsupportedField, row_path));
            }
            let children = group
                .iter()
                .enumerate()
                .map(|(index, row)| {
                    encode_stack_row(
                        row,
                        &pointer(&row_fields, &index.to_string()),
                        depth,
                        budget,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(json!({
                "class": "Math::Function::Msgroup",
                "fields": {"parameter_one": children},
            }))
        }
    }
}

/// Unwinds the nested Stackrel chain into stack rows: `parameter_two`
/// carries the row, `parameter_one` the rest of the stack.
fn decode_stack_chain(
    fields: &Map<String, Value>,
    fields_path: &str,
    depth: usize,
    budget: &mut Budget,
) -> Result<Vec<StackRow>, WireError> {
    let under_path = pointer(fields_path, "parameter_one");
    let over = required(fields, "parameter_two", fields_path)?;
    let mut rows = Vec::new();
    if !over.is_null() {
        rows.push(decode_stack_row(
            over,
            &pointer(fields_path, "parameter_two"),
            depth,
            budget,
        )?);
    }
    match required(fields, "parameter_one", fields_path)? {
        Value::Null => Ok(rows),
        under @ Value::Object(_) => {
            let under_path = under_path.clone();
            let under_fields = required(
                under.as_object().expect("object checked above"),
                "fields",
                &under_path,
            )?
            .as_object()
            .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &pointer(&under_path, "fields")))?
            .clone();
            let under_class = required(
                under.as_object().expect("object checked above"),
                "class",
                &under_path,
            )?
            .as_str()
            .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &pointer(&under_path, "class")))?
            .to_owned();
            if under_class != "Math::Function::Stackrel" {
                return Err(refusal(
                    WireErrorCode::UnsupportedClass,
                    &pointer(&under_path, "class"),
                ));
            }
            let mut under_rows = decode_stack_chain(
                &under_fields,
                &pointer(&under_path, "fields"),
                depth,
                budget,
            )?;
            rows.append(&mut under_rows);
            Ok(rows)
        }
        _ => Err(refusal(WireErrorCode::InvalidValue, &under_path)),
    }
}

/// One stack row off the wire: a plain formula sequence, an Msline, or a
/// Msgroup of rows.
fn decode_stack_row(
    value: &Value,
    path: &str,
    depth: usize,
    budget: &mut Budget,
) -> Result<StackRow, WireError> {
    let object = value
        .as_object()
        .ok_or_else(|| refusal(WireErrorCode::InvalidValue, path))?;
    let class = required(object, "class", path)?
        .as_str()
        .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &pointer(path, "class")))?;
    let fields_path = pointer(path, "fields");
    match class {
        "Math::Formula" => {
            let fields = required(object, "fields", path)?
                .as_object()
                .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &fields_path))?;
            closed(fields, &["value"], &fields_path)?;
            let value_path = pointer(&fields_path, "value");
            let children = required(fields, "value", &fields_path)?
                .as_array()
                .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &value_path))?;
            let cells = children
                .iter()
                .enumerate()
                .map(|(index, child)| {
                    let child_path = pointer(&value_path, &index.to_string());
                    decode_node(
                        child
                            .as_object()
                            .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &child_path))?,
                        &child_path,
                        depth,
                        budget,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(StackRow::Cells {
                position: None,
                cells,
            })
        }
        "Math::Function::Msline" => {
            let fields = required(object, "fields", path)?
                .as_object()
                .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &fields_path))?;
            closed(fields, &["parameter_one"], &fields_path)?;
            if !required(fields, "parameter_one", &fields_path)?.is_null() {
                return Err(refusal(
                    WireErrorCode::UnsupportedField,
                    &pointer(&fields_path, "parameter_one"),
                ));
            }
            Ok(StackRow::Line {
                position: None,
                length: None,
            })
        }
        "Math::Function::Msgroup" => {
            let fields = required(object, "fields", path)?
                .as_object()
                .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &fields_path))?;
            closed(fields, &["parameter_one", "options"], &fields_path)?;
            let value_path = pointer(&fields_path, "parameter_one");
            let children = required(fields, "parameter_one", &fields_path)?
                .as_array()
                .ok_or_else(|| refusal(WireErrorCode::InvalidValue, &value_path))?;
            let rows = children
                .iter()
                .enumerate()
                .map(|(index, child)| {
                    decode_stack_row(
                        child,
                        &pointer(&value_path, &index.to_string()),
                        depth,
                        budget,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(StackRow::Group {
                position: None,
                shift: None,
                rows,
            })
        }
        _ => Err(refusal(
            WireErrorCode::UnsupportedClass,
            &pointer(path, "class"),
        )),
    }
}
