//! The typed Formula tree: logifos's rendering model, converted at one
//! boundary from the plurimath common model (`{class: "Math::*", fields}`).
//!
//! Formula is the model; syntax models transform into it and back. The
//! layout and rendering stages consume only this tree, so adding a syntax
//! never touches them.

mod wire;
pub use wire::{ContractPin, WireError, WireErrorCode, FORMULA_CONTRACT, MAX_DEPTH, MAX_NODES};

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Map, Value};

    #[test]
    #[should_panic(expected = "UnsupportedField")]
    fn refuses_bootstrap_sequence_spelling() {
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
