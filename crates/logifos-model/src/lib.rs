//! The typed Formula tree: logifos's rendering model, converted at one
//! boundary from the plurimath common model (`{class: "Math::*", fields}`).
//!
//! Formula is the model; syntax models transform into it and back. The
//! layout and rendering stages consume only this tree, so adding a syntax
//! never touches them.

mod wire;
pub use wire::{ContractPin, WireError, WireErrorCode, FORMULA_CONTRACT, MAX_DEPTH, MAX_NODES};

/// The typed formula node shared by syntax, layout and wire conversion.
/// A stage refuses variants it cannot represent without losing information.
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
    /// Base, subscript and superscript in source order.
    SubSup(Box<Formula>, Box<Formula>, Box<Formula>),
    /// Base with ordered postscript pairs and an optional prescript boundary.
    MultiScripts {
        /// The scripted base.
        base: Box<Formula>,
        /// Pairs after the base, in source order.
        postscripts: Vec<ScriptPair>,
        /// Pairs after `mprescripts`; `Some([])` preserves an empty boundary.
        prescripts: Option<Vec<ScriptPair>>,
    },
    /// An attribute-free MathML space with default dimensions.
    Space,
    /// A string literal, distinct from identifier text and with default quotes.
    StringLiteral(String),
    /// Base with an underscript child, distinct from a subscript.
    Under(Box<Formula>, Box<Formula>),
    /// Base with an overscript child, distinct from a superscript.
    Over(Box<Formula>, Box<Formula>),
    /// Base, underscript and overscript in MathML source order.
    UnderOver(Box<Formula>, Box<Formula>, Box<Formula>),
    /// A square root around one radicand child.
    Sqrt(Box<Formula>),
    /// A radicand with an explicit index child.
    Root(Box<Formula>, Box<Formula>),
    /// An operator symbol with an implied spacing class.
    Operator(String),
    /// A text or identifier symbol.
    Symbol(String),
}

/// One ordered subscript/superscript pair in a multiscript construct.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptPair {
    /// `None` preserves an explicit `none` placeholder, not an empty row.
    pub subscript: Option<Box<Formula>>,
    /// `None` preserves an explicit `none` placeholder, not an empty row.
    pub superscript: Option<Box<Formula>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Map, Value};

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

    #[test]
    fn radical_wire_shapes_match_the_upstream_fixtures() {
        // The portable fixture shape: Sqrt carries only parameter_one; Root
        // keeps the radicand in parameter_one and the index in parameter_two.
        let radicand = json!({"class": "Math::Symbols::Symbol", "fields": {"value": "x"}});
        let index = json!({"class": "Math::Symbols::Symbol", "fields": {"value": "3"}});
        let sqrt = json!({"class": "Math::Function::Sqrt", "fields": {"parameter_one": radicand}});
        let formula = Formula::from_wire(sqrt.as_object().unwrap()).unwrap();
        assert_eq!(
            formula,
            Formula::Sqrt(Box::new(Formula::Symbol("x".into())))
        );
        assert_eq!(formula.to_wire().unwrap(), sqrt);
        let root = json!({
            "class": "Math::Function::Root",
            "fields": {"parameter_one": radicand, "parameter_two": index},
        });
        let formula = Formula::from_wire(root.as_object().unwrap()).unwrap();
        assert_eq!(
            formula,
            Formula::Root(
                Box::new(Formula::Symbol("x".into())),
                Box::new(Formula::Symbol("3".into())),
            )
        );
        assert_eq!(formula.to_wire().unwrap(), root);
    }

    #[test]
    fn radicals_refuse_fields_the_tree_cannot_carry() {
        let model = json!({
            "class": "Math::Function::Sqrt",
            "fields": {
                "parameter_one": {"class": "Math::Symbols::Symbol", "fields": {"value": "x"}},
                "hide_function_name": false,
            },
        });
        assert!(Formula::from_wire(model.as_object().unwrap()).is_err());
        let model = json!({
            "class": "Math::Function::Root",
            "fields": {"parameter_one": {"class": "Math::Symbols::Symbol", "fields": {"value": "x"}}},
        });
        assert!(Formula::from_wire(model.as_object().unwrap()).is_err());
    }
}
