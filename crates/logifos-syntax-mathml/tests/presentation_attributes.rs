use logifos_model::{Formula, MathDisplay, PresentationStyle, ScriptLevel};
use logifos_syntax_mathml::{parse, transform, write, TransformErrorKind, WriteErrorKind};

#[test]
fn display_and_script_hints_roundtrip_with_canonical_values() {
    let source = "<math display='BLOCK' displaystyle='false' scriptlevel='01'><mstyle displaystyle='TRUE' scriptlevel='+2'><mrow scriptlevel='-3'><mi>x</mi></mrow></mstyle></math>";
    let formula = transform(&parse(source).unwrap()).unwrap();
    let Formula::Styled { style, content } = &formula else {
        panic!("missing root style")
    };
    assert_eq!(
        *style,
        PresentationStyle {
            display: Some(MathDisplay::Block),
            displaystyle: Some(false),
            script_level: Some(ScriptLevel::Absolute(1))
        }
    );
    assert!(matches!(content.as_ref(), Formula::Sequence(_)));
    let output = write(&formula).unwrap();
    assert!(output.contains("display=\"block\" displaystyle=\"false\" scriptlevel=\"1\""));
    assert!(output.contains("displaystyle=\"true\" scriptlevel=\"+2\""));
    assert!(output.contains("scriptlevel=\"-3\""));
    assert_eq!(transform(&parse(output).unwrap()).unwrap(), formula);
}

#[test]
fn grouping_elements_keep_their_owning_style() {
    for name in ["mrow", "mstyle", "merror", "mpadded", "mphantom"] {
        let source = format!(
            "<math><{name} displaystyle='true' scriptlevel='255'><mi>x</mi></{name}></math>"
        );
        let formula = transform(&parse(&source).unwrap()).unwrap();
        let output = write(&formula).unwrap();
        assert!(output.contains(&format!(
            "<{name} displaystyle=\"true\" scriptlevel=\"255\">"
        )));
        assert_eq!(transform(&parse(output).unwrap()).unwrap(), formula);
    }
}

#[test]
fn invalid_attributes_keep_typed_refusals_and_exact_paths() {
    for attr in [
        "display='wide'",
        "displaystyle='1'",
        "scriptlevel='256'",
        "scriptlevel='-256'",
        "scriptlevel='1.0'",
        "scriptlevel='+'",
        "scriptlevel=' 1'",
        "scriptlevel='١'",
        "mathcolor='red'",
        "f:display='block' xmlns:f='urn:foreign'",
    ] {
        let source = format!("<math {attr}><mi>x</mi></math>");
        let error = transform(&parse(&source).unwrap()).unwrap_err();
        assert_eq!(
            error.kind,
            TransformErrorKind::UnsupportedAttribute,
            "{source}"
        );
        assert_eq!(error.location, "/root/attributes/0", "{source}");
        assert!(error.attribute.is_some());
    }
    for body in [
        "<mrow display='block'/>",
        "<mi displaystyle='true'>x</mi>",
        "<semantics displaystyle='true'><mi>x</mi></semantics>",
    ] {
        let error = transform(&parse(format!("<math>{body}</math>")).unwrap()).unwrap_err();
        assert_eq!(error.kind, TransformErrorKind::UnsupportedAttribute);
        assert_eq!(error.location, "/root/children/0/attributes/0");
    }
}

#[test]
fn table_rows_and_cells_refuse_attributes_instead_of_losing_them() {
    for (source, path) in [
        (
            "<math><mtable><mtr displaystyle='true'><mtd/></mtr></mtable></math>",
            "/root/children/0/children/0/attributes/0",
        ),
        (
            "<math><mtable><mtr><mtd columnalign='center'/></mtr></mtable></math>",
            "/root/children/0/children/0/children/0/attributes/0",
        ),
    ] {
        let error = transform(&parse(source).unwrap()).unwrap_err();
        assert_eq!(error.kind, TransformErrorKind::UnsupportedAttribute);
        assert_eq!(error.location, path);
    }
}

#[test]
fn writer_refuses_invalid_styled_shapes() {
    let style = PresentationStyle {
        display: Some(MathDisplay::Block),
        ..Default::default()
    };
    for formula in [
        Formula::Styled {
            style,
            content: Box::new(Formula::Symbol("x".into())),
        },
        Formula::Sequence(vec![Formula::Styled {
            style,
            content: Box::new(Formula::Sequence(vec![])),
        }]),
        Formula::Styled {
            style: PresentationStyle::default(),
            content: Box::new(Formula::Sequence(vec![])),
        },
    ] {
        assert_eq!(
            write(&formula).unwrap_err().kind,
            WriteErrorKind::InvalidStructure
        );
    }
}
