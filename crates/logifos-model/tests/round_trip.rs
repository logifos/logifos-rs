//! Independent wire and typed-tree round trips, including refusal atomicity.

use logifos_model::{Formula, WireError, WireErrorCode, MAX_DEPTH, MAX_NODES};
use serde_json::{json, Value};

fn symbol(text: &str) -> Value {
    json!({"class": "Math::Symbols::Symbol", "fields": {"value": text}})
}

fn sequence(children: Vec<Value>) -> Value {
    json!({"class": "Math::Formula", "fields": {"value": children}})
}

fn binary(class: &str, first: Value, second: Value) -> Value {
    json!({"class": class, "fields": {"parameter_one": first, "parameter_two": second}})
}

fn round_trip(wire: &Value, expected: &Formula) {
    let snapshot = wire.clone();
    let typed = Formula::try_from(wire).unwrap();
    assert_eq!(&typed, expected);
    let output = typed.to_wire().unwrap();
    assert_eq!(
        &output, wire,
        "every field, value, class and child must survive"
    );
    assert_eq!(Formula::try_from(&output).unwrap(), typed);
    assert_eq!(wire, &snapshot, "input must remain unchanged");
    assert_eq!(
        Formula::from_wire(wire.as_object().unwrap()).unwrap(),
        typed
    );
}

fn round_trips(wire: &Value) {
    let typed = Formula::try_from(wire).expect("wire decodes");
    let output = typed.to_wire().expect("model re-encodes");
    assert_eq!(&output, wire);
}

fn refused(wire: &Value, code: WireErrorCode, path: &str) {
    let snapshot = wire.clone();
    assert_eq!(
        Formula::try_from(wire),
        Err(WireError {
            code,
            path: path.to_owned()
        }),
    );
    assert_eq!(wire, &snapshot, "refusal must leave the full input intact");
}

#[test]
fn nonempty_round_trip_preserves_scalar_bytes_and_child_order() {
    let texts = ["", "  x\n", "a\0b", "e\u{301}", "é", "∑𝑥", "<>&"];
    round_trip(
        &sequence(texts.iter().map(|text| symbol(text)).collect()),
        &Formula::Sequence(
            texts
                .iter()
                .map(|text| Formula::Symbol((*text).into()))
                .collect(),
        ),
    );
    round_trip(&sequence(vec![]), &Formula::Sequence(vec![]));
}

#[test]
fn nested_round_trip_covers_all_six_variants() {
    let wire = sequence(vec![
        binary(
            "Math::Function::Frac",
            symbol("numerator"),
            symbol("denominator"),
        ),
        binary(
            "Math::Function::Power",
            symbol("base"),
            sequence(vec![symbol("exponent")]),
        ),
        binary("Math::Function::Base", symbol("indexed"), symbol("index")),
        json!({"class": "Math::Symbols::Plus", "fields": {"value": null}}),
    ]);
    round_trip(
        &wire,
        &Formula::Sequence(vec![
            Formula::Fraction(
                Box::new(Formula::Symbol("numerator".into())),
                Box::new(Formula::Symbol("denominator".into())),
            ),
            Formula::Superscript(
                Box::new(Formula::Symbol("base".into())),
                Box::new(Formula::Sequence(vec![Formula::Symbol("exponent".into())])),
            ),
            Formula::Subscript(
                Box::new(Formula::Symbol("indexed".into())),
                Box::new(Formula::Symbol("index".into())),
            ),
            Formula::Operator("+".into()),
        ]),
    );
}

#[test]
fn named_operator_round_trips_preserve_class_identity_and_explicit_null() {
    for (name, text) in [
        ("Plus", "+"),
        ("Minus", "-"),
        ("Equal", "="),
        ("Asterisk", "*"),
        ("Times", "×"),
    ] {
        round_trip(
            &json!({"class": format!("Math::Symbols::{name}"), "fields": {"value": null}}),
            &Formula::Operator(text.into()),
        );
        round_trip(&symbol(text), &Formula::Symbol(text.into()));
    }
}

#[test]
fn malformed_round_trips_refuse_instead_of_coercing_or_defaulting() {
    for invalid in [
        Value::Null,
        json!(true),
        json!(42),
        json!("node"),
        json!([]),
    ] {
        refused(&invalid, WireErrorCode::InvalidValue, "");
        refused(
            &json!({"class": invalid, "fields": {}}),
            if invalid.is_string() {
                WireErrorCode::UnsupportedClass
            } else {
                WireErrorCode::InvalidValue
            },
            "/class",
        );
        refused(
            &json!({"class": "Math::Formula", "fields": invalid}),
            WireErrorCode::InvalidValue,
            "/fields",
        );
    }
    refused(&json!({}), WireErrorCode::MissingField, "/class");
    refused(
        &json!({"class": "Math::Formula"}),
        WireErrorCode::MissingField,
        "/fields",
    );
    refused(
        &json!({"class": "Math::Formula", "fields": {}}),
        WireErrorCode::MissingField,
        "/fields/value",
    );
    for value in [Value::Null, json!(false), json!(1), json!("x"), json!({})] {
        refused(
            &json!({"class": "Math::Formula", "fields": {"value": value}}),
            WireErrorCode::InvalidValue,
            "/fields/value",
        );
    }
    for value in [Value::Null, json!(false), json!(1), json!([]), json!({})] {
        refused(
            &json!({"class": "Math::Symbols::Symbol", "fields": {"value": value}}),
            WireErrorCode::InvalidValue,
            "/fields/value",
        );
    }
}

#[test]
fn unknown_class_round_trips_refuse_at_any_depth() {
    for class in [
        "",
        "Math::Unknown",
        "Math::Number",
        "Math::Fraction",
        "Math::Symbol",
        "Other::Symbol",
        "Other::Function::Frac",
        "Math::Symbols::Symbol::Unknown",
    ] {
        let wire = json!({"class": class, "fields": {"value": "must survive"}});
        refused(&wire, WireErrorCode::UnsupportedClass, "/class");
        refused(
            &sequence(vec![
                symbol("before"),
                sequence(vec![wire]),
                symbol("after"),
            ]),
            WireErrorCode::UnsupportedClass,
            "/fields/value/1/fields/value/0/class",
        );
    }
}

#[test]
fn binary_round_trips_reject_missing_null_and_nonnode_operands() {
    for class in [
        "Math::Function::Frac",
        "Math::Function::Power",
        "Math::Function::Base",
    ] {
        for key in ["parameter_one", "parameter_two"] {
            let mut wire = binary(class, symbol("left"), symbol("right"));
            wire["fields"].as_object_mut().unwrap().remove(key);
            refused(
                &wire,
                WireErrorCode::MissingField,
                &format!("/fields/{key}"),
            );
            for invalid in [Value::Null, json!(true), json!(1), json!("text"), json!([])] {
                wire["fields"][key] = invalid;
                refused(
                    &wire,
                    WireErrorCode::InvalidValue,
                    &format!("/fields/{key}"),
                );
            }
        }
    }
}

#[test]
fn every_supported_shape_refuses_additional_information_even_when_null() {
    let mut nodes = vec![sequence(vec![symbol("child")]), symbol("text")];
    for class in [
        "Math::Function::Frac",
        "Math::Function::Power",
        "Math::Function::Base",
    ] {
        nodes.push(binary(class, symbol("left"), symbol("right")));
    }
    for name in ["Plus", "Minus", "Equal", "Asterisk", "Times"] {
        nodes.push(json!({"class": format!("Math::Symbols::{name}"), "fields": {"value": null}}));
    }
    for node in nodes {
        for value in [
            Value::Null,
            json!({}),
            json!([]),
            json!(false),
            json!("retained"),
        ] {
            for key in ["attributes", "options", "future~field/data"] {
                for envelope in [true, false] {
                    let mut wire = node.clone();
                    let (object, prefix) = if envelope {
                        (wire.as_object_mut().unwrap(), "")
                    } else {
                        (wire["fields"].as_object_mut().unwrap(), "/fields")
                    };
                    object.insert(key.into(), value.clone());
                    let pointer = format!("{prefix}/{}", key.replace('~', "~0").replace('/', "~1"));
                    refused(&wire, WireErrorCode::UnsupportedField, &pointer);
                    refused(
                        &sequence(vec![symbol("before"), wire, symbol("after")]),
                        WireErrorCode::UnsupportedField,
                        &format!("/fields/value/1{pointer}"),
                    );
                }
            }
        }
    }
}

#[test]
fn upstream_optional_metadata_is_refused_even_when_null() {
    for metadata in [
        Value::Null,
        json!({}),
        json!({"schema": "ruby_values_v1", "fields": {}}),
    ] {
        let mut wire = symbol("unmodified");
        wire["source_values"] = metadata;
        refused(&wire, WireErrorCode::UnsupportedField, "/source_values");
    }
}

#[test]
fn named_operators_refuse_missing_or_overridden_values() {
    for name in ["Plus", "Minus", "Equal", "Asterisk", "Times"] {
        let class = format!("Math::Symbols::{name}");
        refused(
            &json!({"class": class, "fields": {}}),
            WireErrorCode::MissingField,
            "/fields/value",
        );
        for value in [json!("+"), json!(false), json!(1), json!([]), json!({})] {
            refused(
                &json!({"class": class, "fields": {"value": value}}),
                WireErrorCode::InvalidValue,
                "/fields/value",
            );
        }
    }
}

#[test]
fn arbitrary_operators_ride_the_generic_symbol_class() {
    // plurimath's own tree carries any operator spelling no Symbols class
    // claims as the generic Symbols::Symbol holding the text, so the wire
    // mirrors that instead of refusing.
    for text in ["", "arbitrary"] {
        let node = Formula::Operator(text.into());
        let wire = node.to_wire().unwrap();
        assert_eq!(wire["class"], "Math::Symbols::Symbol");
        assert_eq!(wire["fields"]["value"], text);
        let decoded = Formula::from_wire(wire.as_object().unwrap()).unwrap();
        assert_eq!(decoded, Formula::Symbol(text.into()));
    }
}
#[test]
fn bootstrap_field_alias_is_refused_instead_of_reinterpreted() {
    refused(
        &json!({"class": "Math::Formula", "fields": {"parameter1": []}}),
        WireErrorCode::UnsupportedField,
        "/fields/parameter1",
    );
    refused(
        &json!({"class": "Math::Function::Frac", "fields": {"parameter1": symbol("a"), "parameter2": symbol("b")}}),
        WireErrorCode::UnsupportedField,
        "/fields/parameter1",
    );
}

#[test]
fn exact_depth_limit_round_trips_and_the_next_level_refuses_both_directions() {
    // The recursion at the exact depth limit needs frames the default test
    // stack cannot spare.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(exact_depth_limit_body)
        .expect("test thread spawns")
        .join()
        .expect("test thread succeeds");
}

fn exact_depth_limit_body() {
    let mut wire = symbol("leaf");
    let mut typed = Formula::Symbol("leaf".into());
    for _ in 1..MAX_DEPTH {
        wire = sequence(vec![wire]);
        typed = Formula::Sequence(vec![typed]);
    }
    round_trip(&wire, &typed);
    let too_deep = sequence(vec![wire]);
    assert_eq!(
        Formula::try_from(&too_deep).unwrap_err().code,
        WireErrorCode::ResourceLimit
    );
    assert_eq!(
        Formula::Sequence(vec![typed]).to_wire().unwrap_err().code,
        WireErrorCode::ResourceLimit
    );
}

#[test]
fn exact_node_limit_round_trips_and_the_next_node_refuses_both_directions() {
    let mut typed = Formula::Sequence(vec![Formula::Symbol("n".into()); MAX_NODES - 1]);
    let wire = typed.to_wire().unwrap();
    assert_eq!(Formula::try_from(&wire).unwrap(), typed);
    if let Formula::Sequence(children) = &mut typed {
        children.push(Formula::Symbol("overflow".into()));
    }
    assert_eq!(
        typed.to_wire().unwrap_err().code,
        WireErrorCode::ResourceLimit
    );
    let mut too_wide = wire;
    too_wide["fields"]["value"]
        .as_array_mut()
        .unwrap()
        .push(symbol("overflow"));
    assert_eq!(
        Formula::try_from(&too_wide).unwrap_err().code,
        WireErrorCode::ResourceLimit
    );
}

#[test]
fn upstream_constructor_information_is_refused_even_when_default_valued() {
    // left_right_wrapper stays constructor information with no retained
    // meaning. displaystyle moved to the retained-hint class and round
    // trips (see root_display_hint_round_trips_below).
    for value in [json!(true), json!(false), Value::Null] {
        let mut wire = sequence(vec![symbol("kept")]);
        wire["fields"]["left_right_wrapper"] = value.clone();
        refused(
            &wire,
            WireErrorCode::UnsupportedField,
            "/fields/left_right_wrapper",
        );
        refused(
            &sequence(vec![symbol("before"), wire, symbol("after")]),
            WireErrorCode::UnsupportedField,
            "/fields/value/1/fields/left_right_wrapper",
        );
    }
    let wire = json!({
        "class": "Math::Formula",
        "fields": {"value": [symbol("kept")], "displaystyle": true, "left_right_wrapper": true}
    });
    refused(
        &wire,
        WireErrorCode::UnsupportedField,
        "/fields/left_right_wrapper",
    );
}

#[test]
fn root_display_hint_round_trips() {
    for displaystyle in [true, false] {
        let wire = json!({
            "class": "Math::Formula",
            "fields": {"value": [symbol("kept")], "displaystyle": displaystyle}
        });
        round_trips(&wire);
    }
    let wire = json!({
        "class": "Math::Formula",
        "fields": {"value": [symbol("kept")], "displaystyle": Value::Null}
    });
    refused(&wire, WireErrorCode::InvalidValue, "/fields/displaystyle");
}

#[test]
fn presentation_attributes_beyond_the_display_hint_refuse() {
    for (field, value) in [
        ("display", json!("block")),
        ("scriptlevel", json!(1)),
        ("mathvariant", json!("bold")),
        ("mathcolor", json!("#112233")),
    ] {
        let wire = json!({
            "class": "Math::Formula",
            "fields": {"value": [symbol("kept")], field: value}
        });
        refused(
            &wire,
            WireErrorCode::UnsupportedField,
            &format!("/fields/{field}"),
        );
    }
}

#[test]
fn construct_classes_round_trip_and_preserve_children() {
    let base = symbol("x");
    let script = symbol("n");
    let under = json!({
        "class": "Math::Function::Underset",
        "fields": {"parameter_one": base, "parameter_two": script}
    });
    round_trips(&under);
    let over = json!({
        "class": "Math::Function::Overset",
        "fields": {"parameter_one": base, "parameter_two": script}
    });
    round_trips(&over);
    let underover = json!({
        "class": "Math::Function::Underover",
        "fields": {
            "parameter_one": base, "parameter_two": script, "parameter_three": symbol("k")
        }
    });
    round_trips(&underover);
    let power_base = json!({
        "class": "Math::Function::PowerBase",
        "fields": {
            "parameter_one": base, "parameter_two": script, "parameter_three": symbol("k")
        }
    });
    round_trips(&power_base);
    let mstyle = json!({
        "class": "Math::Formula::Mstyle",
        "fields": {"value": [symbol("kept")]}
    });
    round_trips(&mstyle);
    for class in [
        "Math::Function::Merror",
        "Math::Function::Mpadded",
        "Math::Function::Phantom",
    ] {
        let group = json!({
            "class": class,
            "fields": {"parameter_one": [symbol("a"), symbol("b")]}
        });
        round_trips(&group);
    }
    let literal = json!({
        "class": "Math::Function::Ms",
        "fields": {"parameter_one": "quoted"}
    });
    round_trips(&literal);
}

#[test]
fn font_style_and_multiscript_round_trip() {
    // A token's variant crosses as its plurimath FontStyle class with the
    // canonical spelling as the scalar second parameter.
    let styled = json!({
        "class": "Math::Function::FontStyle::Bold",
        "fields": {"parameter_one": symbol("x"), "parameter_two": "bold"}
    });
    round_trips(&styled);
    // A misspelled parameter refuses rather than re-coercing.
    let mismatched = json!({
        "class": "Math::Function::FontStyle::Bold",
        "fields": {"parameter_one": symbol("x"), "parameter_two": "italic"}
    });
    refused(
        &mismatched,
        WireErrorCode::InvalidValue,
        "/fields/parameter_two",
    );

    // Postscripts nest into the base as a PowerBase over the parallel
    // lists; prescripts are the ternary's parallel parameters.
    let multiscript = json!({
        "class": "Math::Function::Multiscript",
        "fields": {
            "parameter_one": {
                "class": "Math::Function::PowerBase",
                "fields": {
                    "parameter_one": symbol("x"),
                    "parameter_two": symbol("a"),
                    "parameter_three": symbol("b")
                }
            },
            "parameter_two": symbol("c"),
            "parameter_three": symbol("d")
        }
    });
    round_trips(&multiscript);
    // Without prescripts the ternary's trailing parameters are null.
    let posts_only = json!({
        "class": "Math::Function::Multiscript",
        "fields": {
            "parameter_one": symbol("x"),
            "parameter_two": Value::Null,
            "parameter_three": Value::Null
        }
    });
    round_trips(&posts_only);
}

#[test]
fn color_and_accent_round_trip() {
    // Color wraps the content innermost as a Color over a Text carrying the
    // name (plurimath's own wrapping order).
    let colored = json!({
        "class": "Math::Function::Color",
        "fields": {
            "parameter_one": {
                "class": "Math::Function::Text",
                "fields": {"parameter_one": "red"}
            },
            "parameter_two": symbol("x")
        }
    });
    round_trips(&colored);
    // An unknown color name refuses rather than re-coercing.
    let unknown = json!({
        "class": "Math::Function::Color",
        "fields": {
            "parameter_one": {
                "class": "Math::Function::Text",
                "fields": {"parameter_one": "chartreuse"}
            },
            "parameter_two": symbol("x")
        }
    });
    refused(
        &unknown,
        WireErrorCode::UnsupportedClass,
        "/fields/parameter_one/parameter_one",
    );

    // Accent rides the under/over construct's options hash.
    let accented = json!({
        "class": "Math::Function::Overset",
        "fields": {
            "parameter_one": symbol("x"),
            "parameter_two": symbol("."),
            "options": {"accent": true}
        }
    });
    round_trips(&accented);
    let unaccented = json!({
        "class": "Math::Function::Overset",
        "fields": {
            "parameter_one": symbol("x"),
            "parameter_two": symbol("."),
            "options": {"accent": false}
        }
    });
    round_trips(&unaccented);
    // A foreign option key refuses.
    let foreign = json!({
        "class": "Math::Function::Overset",
        "fields": {
            "parameter_one": symbol("x"),
            "parameter_two": symbol("."),
            "options": {"align": "center"}
        }
    });
    refused(&foreign, WireErrorCode::UnsupportedField, "/options/align");
}

#[test]
fn authored_space_lengths_round_trip_through_named_ones() {
    // An authored em length carries as the Em variant; the writer renders it
    // as its em spelling and the named spaces keep their spellings.
    let space = logifos_model::MathSpace::Em(-0.35);
    assert_eq!(space.units(), -6.3);
    assert_eq!(space.to_css(), "-0.35em");
    assert_eq!(
        logifos_model::MathSpace::parse("0.2778em"),
        Some(logifos_model::MathSpace::Em(0.2778))
    );
    assert_eq!(
        logifos_model::MathSpace::parse("thickmathspace"),
        Some(logifos_model::MathSpace::Thick)
    );
    assert_eq!(logifos_model::MathSpace::parse("notalength"), None);
}

#[test]
fn menclose_round_trips_with_its_verbatim_notation() {
    let menclose = json!({
        "class": "Math::Function::Menclose",
        "fields": {
            "parameter_one": "circle box",
            "parameter_two": symbol("x")
        }
    });
    round_trips(&menclose);
}
