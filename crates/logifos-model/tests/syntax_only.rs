use logifos_model::{Formula, PresentationStyle, WireErrorCode};

#[test]
fn unmapped_syntax_nodes_refuse_wire_output_without_losing_content() {
    // Space has no source-class mapping (plurimath drops plain mspace) and
    // a MultiScripts pair with an empty slot has no aligned plurimath
    // representation — both refuse rather than losing their placement.
    let child = Box::new(Formula::Symbol("private".into()));
    let hole = Formula::MultiScripts {
        base: Box::new(Formula::Symbol("base".into())),
        postscripts: vec![logifos_model::ScriptPair {
            subscript: None,
            superscript: Some(child.clone()),
        }],
        prescripts: None,
    };
    let error = hole.to_wire().unwrap_err();
    assert_eq!(error.code, WireErrorCode::UnsupportedClass);
    assert_eq!(error.path, "/fields/parameter_one~1postscripts/0");
    let node = Formula::Space(None, None, None);
    let error = node.to_wire().unwrap_err();
    assert_eq!(error.code, WireErrorCode::UnsupportedClass);
    assert_eq!(error.path, "");
    let error = Formula::Table(vec![vec![Formula::Sequence(vec![node.clone()])]])
        .to_wire()
        .unwrap_err();
    assert_eq!(error.code, WireErrorCode::UnsupportedClass);
    assert_eq!(
        error.path,
        "/fields/value/0/fields~1parameter_one/0/fields~1parameter_one/0"
    );
    let error = Formula::Sequence(vec![Formula::Sequence(vec![]), node])
        .to_wire()
        .unwrap_err();
    assert_eq!(error.code, WireErrorCode::UnsupportedClass);
    assert_eq!(error.path, "/fields/value/1");
}

#[test]
fn mapped_constructs_cross_the_wire_through_containers() {
    // The source-class mappings (Underset/Overset/Underover/PowerBase,
    // Formula::Mstyle, Merror/Mpadded/Phantom, Ms) carry the constructs
    // through nesting, and error paths still never quote content.
    let child = Box::new(Formula::Symbol("private".into()));
    for node in [
        Formula::Under(child.clone(), child.clone()),
        Formula::Over(child.clone(), child.clone()),
        Formula::UnderOver(child.clone(), child.clone(), child.clone()),
        Formula::SubSup(child.clone(), child.clone(), child.clone()),
        Formula::MultiScripts {
            base: child.clone(),
            postscripts: vec![logifos_model::ScriptPair {
                subscript: Some(child.clone()),
                superscript: Some(child.clone()),
            }],
            prescripts: Some(vec![logifos_model::ScriptPair {
                subscript: Some(child.clone()),
                superscript: Some(child.clone()),
            }]),
        },
        Formula::Style(vec![Formula::Symbol("private".into())]),
        Formula::ErrorGroup(vec![Formula::Symbol("private".into())]),
        Formula::Padded(vec![Formula::Symbol("private".into())]),
        Formula::Phantom(vec![Formula::Symbol("private".into())]),
        Formula::StringLiteral("private".into()),
    ] {
        let nested = Formula::Sequence(vec![Formula::Sequence(vec![]), node]);
        let wire = nested.to_wire().expect("mapped construct encodes");
        assert!(
            wire.to_string().contains("private"),
            "the mapping carries the content"
        );
    }
}

#[test]
fn styled_wrappers_cross_the_wire_only_with_the_display_hint() {
    let hint_only = Formula::Styled {
        style: PresentationStyle {
            displaystyle: Some(true),
            ..PresentationStyle::default()
        },
        content: Box::new(Formula::Symbol("x".into())),
    };
    hint_only.to_wire().expect("the display hint round trips");

    // A variant crosses as its FontStyle class; a variant beside another
    // carried hint still refuses.
    let variant_with_hint = Formula::Styled {
        style: PresentationStyle {
            math_variant: Some(logifos_model::MathVariant::Bold),
            displaystyle: Some(true),
            ..PresentationStyle::default()
        },
        content: Box::new(Formula::Symbol("x".into())),
    };
    let error = variant_with_hint.to_wire().unwrap_err();
    assert_eq!(error.code, WireErrorCode::UnsupportedClass);
}
