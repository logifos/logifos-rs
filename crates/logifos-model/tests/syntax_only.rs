use logifos_model::{Formula, PresentationStyle, WireErrorCode};

#[test]
fn unmapped_syntax_nodes_refuse_wire_output_without_losing_content() {
    // MultiScripts and Space have no source-class mapping yet — Multiscript's
    // pair layout and mspace extents are not pinned upstream — so they refuse
    // rather than inventing classes or losing their placement.
    let child = Box::new(Formula::Symbol("private".into()));
    for node in [
        Formula::MultiScripts {
            base: child.clone(),
            postscripts: vec![logifos_model::ScriptPair {
                subscript: None,
                superscript: Some(child.clone()),
            }],
            prescripts: Some(vec![logifos_model::ScriptPair {
                subscript: Some(child),
                superscript: None,
            }]),
        },
        Formula::Space(None, None, None),
    ] {
        let error = node.to_wire().unwrap_err();
        assert_eq!(error.code, WireErrorCode::UnsupportedClass);
        assert_eq!(error.path, "");
        assert!(!error.to_string().contains("private"));
        let error = Formula::Table(vec![vec![Formula::Sequence(vec![node.clone()])]])
            .to_wire()
            .unwrap_err();
        assert_eq!(error.code, WireErrorCode::UnsupportedClass);
        assert_eq!(
            error.path,
            "/fields/value/0/fields~1parameter_one/0/fields~1parameter_one/0"
        );
        assert!(!error.to_string().contains("private"));
        let error = Formula::Sequence(vec![Formula::Sequence(vec![]), node])
            .to_wire()
            .unwrap_err();
        assert_eq!(error.code, WireErrorCode::UnsupportedClass);
        assert_eq!(error.path, "/fields/value/1");
    }
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

    let variant = Formula::Styled {
        style: PresentationStyle {
            math_variant: Some(logifos_model::MathVariant::Bold),
            ..PresentationStyle::default()
        },
        content: Box::new(Formula::Symbol("x".into())),
    };
    let error = variant.to_wire().unwrap_err();
    assert_eq!(error.code, WireErrorCode::UnsupportedClass);
}
