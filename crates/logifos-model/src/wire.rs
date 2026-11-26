//! Lossless conversion for the pinned upstream common-node subset.

use std::fmt;

use serde_json::{json, Map, Value};

use crate::{Formula, MathDisplay, PresentationStyle};

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
        "\u{27e9}" => Some("Math::Symbols::Paren::Rangle"),
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
                "Math::Function::Underset" => Formula::Under(pair.0, pair.1),
                _ => Formula::Over(pair.0, pair.1),
            })
        }
        "Math::Function::Underover" | "Math::Function::PowerBase" => {
            closed(
                fields,
                &["parameter_one", "parameter_two", "parameter_three"],
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
            Ok(match class {
                "Math::Function::Underover" => Formula::UnderOver(first, second, third),
                _ => Formula::SubSup(first, second, third),
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
        Formula::MultiScripts { .. } | Formula::Space(..) => {
            return Err(refusal(WireErrorCode::UnsupportedClass, path));
        }
        Formula::Styled { style, content } => {
            // The upstream Formula carries only the displaystyle hint. Under
            // MathML Core (§3.1) the display style derives from display or is
            // overridden by displaystyle, so a display hint that AGREES with
            // the carried hint (or is alone) is redundant, not lost — it
            // encodes as the displaystyle. A contradiction refuses. Every
            // other presentation attribute has no source class and stays
            // refused rather than dropped.
            let carries = style.script_level.is_some()
                || style.math_variant.is_some()
                || style.math_color.is_some()
                || style.right_space.is_some()
                || style.accent.is_some()
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
            if carries || displaystyle.is_none() {
                return Err(refusal(WireErrorCode::UnsupportedClass, path));
            }
            let content = encode_node(content, &pointer(&fields_path, "value"), depth + 1, budget)?;
            (
                "Math::Formula",
                json!({"value": [content], "displaystyle": displaystyle}),
            )
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
    };
    Ok(json!({"class": class, "fields": fields}))
}
