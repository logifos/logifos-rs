//! Freshly authored regression cases for the bootstrap wire boundary (MIT).
//! These exercise existing supported shapes, not an upstream mapping contract.

use logifos_model::Formula;
use serde_json::{json, Value};

fn convert(node: Value) -> Result<Formula, String> {
    Formula::from_wire(node.as_object().expect("test node must be an object"))
}

fn sequence(children: Value) -> Value {
    json!({"class": "Math::Formula", "fields": {"parameter1": children}})
}

fn symbol(text: Value) -> Value {
    json!({"class": "Math::Symbol", "fields": {"value": text}})
}

fn assert_refused(node: Value) {
    let error = convert(node.clone()).expect_err(&format!("must refuse {node}"));
    assert!(!error.is_empty(), "refusal must explain the invalid node");
}

#[test]
fn preserves_nonempty_nested_content_and_order() {
    let input = sequence(json!([
        symbol(json!("α")),
        sequence(json!([symbol(json!("+")), symbol(json!("β"))])),
        symbol(json!("tail"))
    ]));
    assert_eq!(
        convert(input).unwrap(),
        Formula::Sequence(vec![
            Formula::Symbol("α".into()),
            Formula::Sequence(vec![
                Formula::Symbol("+".into()),
                Formula::Symbol("β".into()),
            ]),
            Formula::Symbol("tail".into()),
        ])
    );
}

#[test]
fn accepts_an_explicit_empty_sequence() {
    assert_eq!(
        convert(sequence(json!([]))).unwrap(),
        Formula::Sequence(vec![])
    );
}

#[test]
fn preserves_symbol_strings_exactly() {
    for text in ["", "  x\n", "a\u{0}b", "∑𝑥"] {
        assert_eq!(
            convert(symbol(json!(text))).unwrap(),
            Formula::Symbol(text.into())
        );
    }
}

#[test]
fn refuses_a_missing_sequence_field() {
    assert_refused(json!({"class": "Math::Formula", "fields": {}}));
}

#[test]
fn refuses_wrongly_typed_sequence_fields() {
    for value in [
        Value::Null,
        json!(false),
        json!(7),
        json!("x"),
        symbol(json!("x")),
    ] {
        assert_refused(sequence(value));
    }
}

#[test]
fn refuses_nonobject_sequence_children() {
    for child in [Value::Null, json!(true), json!(1), json!("x"), json!([])] {
        assert_refused(sequence(json!([symbol(json!("valid")), child])));
    }
}

#[test]
fn refuses_missing_or_wrongly_typed_envelope_fields() {
    for node in [
        json!({}),
        json!({"fields": {}}),
        json!({"class": "Math::Formula"}),
        json!({"class": "Math::Symbol"}),
    ] {
        assert_refused(node);
    }
    for value in [Value::Null, json!(false), json!(9), json!([]), json!({})] {
        assert_refused(json!({"class": value, "fields": {}}));
    }
    for class in ["Math::Formula", "Math::Symbol"] {
        for fields in [Value::Null, json!(true), json!(9), json!("x"), json!([])] {
            assert_refused(json!({"class": class, "fields": fields}));
        }
    }
}

#[test]
fn refuses_missing_or_wrongly_typed_symbol_values() {
    assert_refused(json!({"class": "Math::Symbol", "fields": {}}));
    for value in [Value::Null, json!(false), json!(1), json!([]), json!({})] {
        assert_refused(symbol(value));
    }
}

#[test]
fn refuses_unexpected_node_information() {
    for mut node in [symbol(json!("x")), sequence(json!([]))] {
        node.as_object_mut()
            .unwrap()
            .insert("style".into(), json!("bold"));
        assert_refused(node);
    }
}

#[test]
fn refuses_unexpected_class_fields() {
    for node in [
        json!({"class": "Math::Formula", "fields": {"parameter1": [], "parameter2": "lost"}}),
        json!({"class": "Math::Formula", "fields": {"parameter1": [], "style": null}}),
        json!({"class": "Math::Symbol", "fields": {"value": "x", "style": "bold"}}),
    ] {
        assert_refused(node);
    }
}

#[test]
fn refuses_unsupported_classes_without_inventing_mappings() {
    for class in ["Math::Unknown", "Math::Fraction", "Math::Operator", ""] {
        assert_refused(json!({"class": class, "fields": {"value": "x"}}));
    }
}

#[test]
fn refuses_invalid_nested_nodes_without_partial_success() {
    for child in [
        json!({"class": "Math::Formula", "fields": {}}),
        sequence(Value::Null),
        symbol(json!(42)),
        json!({"class": "Math::Unknown", "fields": {}}),
        json!({"class": "Math::Symbol", "fields": {"value": "x", "style": "bold"}}),
        json!({"class": "Math::Symbol", "fields": {"value": "x"}, "extra": true}),
    ] {
        assert_refused(sequence(json!([
            symbol(json!("before")),
            sequence(json!([child])),
            symbol(json!("after"))
        ])));
    }
}
