//! The typed Formula tree: logifos's rendering model, converted at one
//! boundary from the plurimath common model (`{class: "Math::*", fields}`).
//!
//! Formula is the model; syntax models transform into it and back. The
//! layout and rendering stages consume only this tree, so adding a syntax
//! never touches them.

use serde_json::{Map, Value};

/// The typed formula node. Variants cover the constructs the layout stage
/// consumes; the wire conversion maps the plurimath `Math::` classes onto
/// them by name.
#[derive(Debug, Clone, PartialEq)]
pub enum Formula {
    /// A horizontal run of child formulas (the plain sequence).
    Sequence(Vec<Formula>),
    /// A fraction: numerator over denominator.
    Fraction(Box<Formula>, Box<Formula>),
    /// Base with a superscript (power) child.
    Superscript(Box<Formula>, Box<Formula>),
    /// Base with a subscript (index) child.
    Subscript(Box<Formula>, Box<Formula>),
    /// An operator symbol with an implied spacing class.
    Operator(String),
    /// A text or identifier symbol.
    Symbol(String),
}

impl Formula {
    /// Converts the plurimath common-model node (`{class, fields}`) into
    /// the typed tree. Only the bootstrap Formula sequence and Symbol
    /// shapes are supported; this is not an authoritative upstream mapping.
    /// Missing, wrongly typed or unexpected information is refused, including
    /// in children, so a successful conversion never silently discards data.
    pub fn from_wire(model: &Map<String, Value>) -> Result<Self, String> {
        let class = model
            .get("class")
            .and_then(Value::as_str)
            .ok_or("model.class must be a string")?;
        let fields = model
            .get("fields")
            .and_then(Value::as_object)
            .ok_or("model.fields must be an object")?;
        Self::check_keys(model, &["class", "fields"], "model")?;
        match class {
            "Math::Formula" => {
                Self::check_keys(fields, &["parameter1"], class)?;
                let children = fields
                    .get("parameter1")
                    .and_then(Value::as_array)
                    .ok_or("Math::Formula.parameter1 must be an array")?;
                let mut nodes = Vec::with_capacity(children.len());
                for value in children {
                    nodes.push(Self::from_wire_value(value)?);
                }
                Ok(Self::Sequence(nodes))
            }
            "Math::Symbol" => {
                Self::check_keys(fields, &["value"], class)?;
                let text = fields
                    .get("value")
                    .and_then(Value::as_str)
                    .ok_or("Math::Symbol needs a string value")?
                    .to_owned();
                Ok(Self::Symbol(text))
            }
            other => Err(format!("unsupported wire class {other:?}")),
        }
    }

    fn check_keys(
        object: &Map<String, Value>,
        allowed: &[&str],
        context: &str,
    ) -> Result<(), String> {
        if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
            return Err(format!("{context} contains unexpected field {key:?}"));
        }
        Ok(())
    }

    fn from_wire_value(value: &Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or("wire model nodes must be objects")?;
        Self::from_wire(object)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_a_sequence_of_symbols() {
        let mut symbol = Map::new();
        symbol.insert("class".into(), Value::from("Math::Symbol"));
        symbol.insert("fields".into(), serde_json::json!({"value": "x"}));
        let mut model = Map::new();
        model.insert("class".into(), Value::from("Math::Formula"));
        model.insert("fields".into(), serde_json::json!({"parameter1": [symbol]}));
        let formula = Formula::from_wire(&model).unwrap();
        assert_eq!(
            formula,
            Formula::Sequence(vec![Formula::Symbol("x".into())])
        );
    }

    #[test]
    fn refuses_unknown_wire_classes() {
        let mut model = Map::new();
        model.insert("class".into(), Value::from("Math::Unknown"));
        model.insert("fields".into(), serde_json::json!({}));
        assert!(Formula::from_wire(&model).is_err());
    }
}
