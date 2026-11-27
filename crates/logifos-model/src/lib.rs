//! The typed Formula tree: logifos's rendering model, converted at one
//! boundary from the plurimath common model (`{class: "Math::*", fields}`).
//!
//! Formula is the model; syntax models transform into it and back. The
//! layout and rendering stages consume only this tree, so adding a syntax
//! never touches them.

pub mod operator;

mod presentation;
pub use presentation::{
    ColumnAlign, MathColor, MathDisplay, MathSpace, MathVariant, PresentationStyle, ScriptLevel,
};

mod operator_wire;
mod wire;
pub use wire::{ContractPin, WireError, WireErrorCode, FORMULA_CONTRACT, MAX_DEPTH, MAX_NODES};

/// The unit of an authored CSS length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
    Em,
    Ex,
    Px,
    /// A CSS point, resolved at the 96dpi default (1pt = 96/72 px).
    Pt,
    /// A percentage: valid on the attribute but never an mspace extent.
    Percent,
}

/// An authored CSS length: value with its unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Length {
    pub value: f64,
    pub unit: LengthUnit,
}

impl Length {
    /// The CSS-pixel extent at a font size and ex height. Percentages
    /// resolve to zero, per Core's mspace presentational-hint rules.
    pub fn px(&self, font_size_px: f64, ex_px: f64) -> f64 {
        match self.unit {
            LengthUnit::Em => self.value * font_size_px,
            LengthUnit::Ex => self.value * ex_px,
            LengthUnit::Px => self.value,
            LengthUnit::Pt => self.value * 96.0 / 72.0,
            LengthUnit::Percent => 0.0,
        }
    }
}

/// The typed formula node shared by syntax, layout and wire conversion.
/// A stage refuses variants it cannot represent without losing information.
#[derive(Debug, Clone, PartialEq)]
pub enum Formula {
    /// A horizontal run of child formulas (the plain sequence).
    Sequence(Vec<Formula>),
    /// Presentation attributes on a retained element. The wrapper
    /// shares its content's syntax depth; nested wrappers are invalid.
    Styled {
        style: PresentationStyle,
        content: Box<Formula>,
    },
    /// An attribute-free MathML style group with inherited presentation.
    Style(Vec<Formula>),
    /// An attribute-free MathML error group.
    ErrorGroup(Vec<Formula>),
    /// An attribute-free MathML padded group with unchanged dimensions.
    Padded(Vec<Formula>),
    /// A row whose contents contribute dimensions but no painted output.
    Phantom(Vec<Formula>),
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
    /// A blank space of authored extents; `None` is an absent attribute,
    /// which Core resolves to zero (Core 3.2.5).
    Space(Option<Length>, Option<Length>, Option<Length>),
    /// A string literal, distinct from identifier text and with default quotes.
    StringLiteral(String),
    /// A MathML-3 enclosure around one content child; the notation is the
    /// element's verbatim attribute value (space-separated spellings),
    /// interpreted at layout.
    Menclose {
        notation: Box<str>,
        content: Box<Formula>,
    },
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
    /// Rows of cells (each an anonymous row) in a table or matrix.
    Table(Vec<Vec<Formula>>),
    /// An operator symbol with an implied spacing class.
    Operator(String),
    /// A text or identifier symbol.
    Symbol(String),
    /// An elementary stack: rows stacked vertically, each a horizontal run
    /// of cells or a horizontal rule, aligned per `stackalign`.
    Stack(StackAlign, Vec<StackRow>),
}

/// The elementary stack's horizontal alignment (`stackalign`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackAlign {
    Left,
    Center,
    Right,
    /// The MathML default: align on the cells' decimal points.
    DecimalPoint,
}

/// One elementary-stack row.
#[derive(Debug, Clone, PartialEq)]
pub enum StackRow {
    /// A horizontal run of cells, offset by `position` columns.
    Cells {
        position: Option<i32>,
        cells: Vec<Formula>,
    },
    /// A subgroup whose rows carry a starting `position` and a per-row
    /// `shift`.
    Group {
        position: Option<i32>,
        shift: Option<i32>,
        rows: Vec<StackRow>,
    },
    /// A horizontal rule under the rows above it, starting at `position`
    /// and spanning `length` columns.
    Line {
        position: Option<i32>,
        length: Option<i32>,
    },
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
    fn stack_wire_chain_roundtrips() {
        let stack = Formula::Stack(
            StackAlign::DecimalPoint,
            vec![
                StackRow::Cells {
                    position: None,
                    cells: vec![Formula::Symbol("424".into())],
                },
                StackRow::Line {
                    position: None,
                    length: None,
                },
                StackRow::Cells {
                    position: None,
                    cells: vec![Formula::Symbol("33".into())],
                },
            ],
        );
        let wire = stack.to_wire().unwrap();
        assert_eq!(wire["class"], "Math::Function::Stackrel");
        // The chain: the top row rides parameter_two, the rest rides
        // parameter_one.
        assert_eq!(
            wire["fields"]["parameter_one"]["class"],
            "Math::Function::Stackrel"
        );
        assert_eq!(
            wire["fields"]["parameter_two"]["fields"]["value"][0]["class"],
            "Math::Symbols::Symbol"
        );
        let decoded = Formula::from_wire(wire.as_object().unwrap()).unwrap();
        assert_eq!(decoded, stack);
    }

    #[test]
    fn stack_align_and_positions_refuse_on_the_wire() {
        let aligned = Formula::Stack(
            StackAlign::Right,
            vec![StackRow::Cells {
                position: None,
                cells: vec![Formula::Symbol("1".into())],
            }],
        );
        assert!(aligned.to_wire().is_err());
        let positioned = Formula::Stack(
            StackAlign::DecimalPoint,
            vec![StackRow::Cells {
                position: Some(1),
                cells: vec![Formula::Symbol("1".into())],
            }],
        );
        assert!(positioned.to_wire().is_err());
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

    #[test]
    fn table_wire_shapes_match_the_upstream_fixtures() {
        let cell = |text: &str| {
            json!({
                "class": "Math::Function::Td",
                "fields": {
                    "parameter_one": [{"class": "Math::Symbols::Symbol", "fields": {"value": text}}],
                    "parameter_two": null,
                },
            })
        };
        let table = json!({
            "class": "Math::Function::Table",
            "fields": {
                "value": [
                    {"class": "Math::Function::Tr", "fields": {"parameter_one": [cell("1"), cell("2")]}},
                    {"class": "Math::Function::Tr", "fields": {"parameter_one": [cell("3"), cell("4")]}},
                ],
                "open_paren": null,
                "close_paren": null,
                "options": {},
            },
        });
        let formula = Formula::from_wire(table.as_object().unwrap()).unwrap();
        let expected_cell = |text: &str| Formula::Sequence(vec![Formula::Symbol(text.into())]);
        assert_eq!(
            formula,
            Formula::Table(vec![
                vec![expected_cell("1"), expected_cell("2")],
                vec![expected_cell("3"), expected_cell("4")],
            ])
        );
        // The minimal emitted shape reparses to the same tree.
        let minimal = formula.to_wire().unwrap();
        assert_eq!(
            Formula::from_wire(minimal.as_object().unwrap()).unwrap(),
            formula
        );
    }

    #[test]
    fn fence_operators_keep_their_paren_classes() {
        let lround = json!({"class": "Math::Symbols::Paren::Lround", "fields": {"value": null}});
        let formula = Formula::from_wire(lround.as_object().unwrap()).unwrap();
        assert_eq!(formula, Formula::Operator("(".into()));
        assert_eq!(formula.to_wire().unwrap(), lround);
        // Both square-bracket classes carry the same character; decoding
        // collapses them, encoding uses the primary spelling.
        for class in ["Lsquare", "Lbrack"] {
            let wire = json!({
                "class": format!("Math::Symbols::Paren::{class}"),
                "fields": {"value": null},
            });
            assert_eq!(
                Formula::from_wire(wire.as_object().unwrap()).unwrap(),
                Formula::Operator("[".into())
            );
        }
        let back = json!({"class": "Math::Symbols::Paren::Lsquare", "fields": {"value": null}});
        assert_eq!(Formula::Operator("[".into()).to_wire().unwrap(), back);
    }

    #[test]
    fn tables_refuse_fences_and_options_the_tree_cannot_carry() {
        let rows = json!({"class": "Math::Function::Tr", "fields": {"parameter_one": []}});
        let fence = json!({
            "class": "Math::Function::Table",
            "fields": {"value": [rows], "open_paren": {"class": "Math::Symbols::Paren::Lsquare", "fields": {"value": null}}},
        });
        assert!(Formula::from_wire(fence.as_object().unwrap()).is_err());
        let styled = json!({
            "class": "Math::Function::Table",
            "fields": {"value": [rows], "options": {"columnlines": "solid"}},
        });
        assert!(Formula::from_wire(styled.as_object().unwrap()).is_err());
        let bare_cell = json!({
            "class": "Math::Function::Table",
            "fields": {"value": [{"class": "Math::Symbols::Symbol", "fields": {"value": "x"}}]},
        });
        assert!(Formula::from_wire(bare_cell.as_object().unwrap()).is_err());
    }
}
