//! Lossless conversion for the pinned upstream common-node subset.

use std::fmt;

use serde_json::{json, Map, Value};

use crate::Formula;

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
fn operator_class(text: &str) -> Option<&'static str> {
    match text {
        "+" => Some("Math::Symbols::Plus"),
        "-" => Some("Math::Symbols::Minus"),
        "=" => Some("Math::Symbols::Equal"),
        "*" => Some("Math::Symbols::Asterisk"),
        "×" => Some("Math::Symbols::Times"),
        _ => None,
    }
}

fn operator_text(class: &str) -> Option<&'static str> {
    match class {
        "Math::Symbols::Plus" => Some("+"),
        "Math::Symbols::Minus" => Some("-"),
        "Math::Symbols::Equal" => Some("="),
        "Math::Symbols::Asterisk" => Some("*"),
        "Math::Symbols::Times" => Some("×"),
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
    match class {
        "Math::Formula" => {
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
            Ok(Formula::Sequence(nodes))
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
        Formula::Under(..)
        | Formula::Over(..)
        | Formula::UnderOver(..)
        | Formula::SubSup(..)
        | Formula::MultiScripts { .. }
        | Formula::Space
        | Formula::StringLiteral(_) => {
            return Err(refusal(WireErrorCode::UnsupportedClass, path));
        }
        Formula::Symbol(text) => ("Math::Symbols::Symbol", json!({"value": text})),
        Formula::Operator(text) => {
            let class = operator_class(text)
                .ok_or_else(|| refusal(WireErrorCode::UnsupportedOperator, path))?;
            (class, json!({"value": null}))
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
    };
    Ok(json!({"class": class, "fields": fields}))
}
