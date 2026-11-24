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
            script_level: Some(ScriptLevel::Absolute(1)),
            ..Default::default()
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
        "mathcolor='chartreuse'",
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
            "<math><mtable><mtr><mtd columnalign='left'/></mtr></mtable></math>",
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

#[test]
fn additional_attribute_families_preserve_values_and_owning_elements() {
    let variants = [
        "normal",
        "bold",
        "italic",
        "bold-italic",
        "double-struck",
        "bold-fraktur",
        "script",
        "bold-script",
        "fraktur",
        "sans-serif",
        "bold-sans-serif",
        "sans-serif-italic",
        "sans-serif-bold-italic",
        "monospace",
    ];
    let spaces = [
        "veryverythinmathspace",
        "verythinmathspace",
        "thinmathspace",
        "mediummathspace",
        "thickmathspace",
        "verythickmathspace",
        "veryverythickmathspace",
    ];
    let mut bodies = Vec::new();
    for value in variants {
        bodies.push(format!(
            "<mstyle mathvariant=\"{value}\"><mi>x</mi></mstyle>"
        ));
    }
    for value in ["red", "blue"] {
        bodies.push(format!("<mstyle mathcolor=\"{value}\"><mi>x</mi><mstyle mathcolor=\"blue\"><mi>y</mi></mstyle></mstyle>"));
    }
    for value in spaces {
        bodies.push(format!("<mo rspace=\"{value}\">+</mo>"));
    }
    for name in ["mover", "munderover"] {
        for value in ["true", "false"] {
            let extra = if name == "munderover" {
                "<mi>z</mi>"
            } else {
                ""
            };
            bodies.push(format!(
                "<{name} accent=\"{value}\"><mi>x</mi><mi>y</mi>{extra}</{name}>"
            ));
        }
    }
    bodies.push("<mtable><mtr><mtd columnalign=\"center\"><mi>x</mi></mtd><mtd columnalign=\"center\"></mtd></mtr></mtable>".into());
    for body in bodies {
        let source = format!("<math>{body}</math>");
        let formula = transform(&parse(&source).unwrap()).unwrap();
        let output = write(&formula).unwrap();
        assert!(output.contains(&body), "{output}");
        assert_eq!(
            transform(&parse(output).unwrap()).unwrap(),
            formula,
            "{source}"
        );
        assert!(formula.to_wire().is_err(), "wire must not lose attributes");
    }
}

#[test]
fn additional_attribute_values_and_wrong_owners_refuse_precisely() {
    for body in [
        "<mstyle mathvariant='unknown'><mi>x</mi></mstyle>",
        "<mstyle mathcolor='red&quot; onclick=&quot;evil'><mi>x</mi></mstyle>",
        "<mo rspace='1em'>+</mo>",
        "<mover accent='yes'><mi>x</mi><mi>y</mi></mover>",
        "<mi mathvariant='bold'>x</mi>",
        "<mstyle rspace='thickmathspace'><mi>x</mi></mstyle>",
        "<mrow accent='true'><mi>x</mi><mi>y</mi></mrow>",
        "<mrow columnalign='center'><mi>x</mi></mrow>",
        "<mo xmlns:f='urn:foreign' f:rspace='thickmathspace'>+</mo>",
    ] {
        let source = format!("<math>{body}</math>");
        let error = transform(&parse(&source).unwrap()).unwrap_err();
        assert_eq!(
            error.kind,
            TransformErrorKind::UnsupportedAttribute,
            "{source}"
        );
        assert_eq!(error.location, "/root/children/0/attributes/0", "{source}");
        assert!(error.attribute.is_some());
    }
    let error = transform(&parse("<math><mover accent='true'><mi>x</mi></mover></math>").unwrap())
        .unwrap_err();
    assert_eq!(error.kind, TransformErrorKind::InvalidStructure);
    assert_eq!(error.location, "/root/children/0");
}

#[test]
fn writer_refuses_attributes_attached_to_wrong_model_constructs() {
    use logifos_model::{ColumnAlign, MathSpace};
    for (style, content) in [
        (
            PresentationStyle {
                right_space: Some(MathSpace::Thick),
                ..Default::default()
            },
            Formula::Sequence(vec![]),
        ),
        (
            PresentationStyle {
                accent: Some(true),
                ..Default::default()
            },
            Formula::Operator("+".into()),
        ),
        (
            PresentationStyle {
                column_align: Some(ColumnAlign::Center),
                ..Default::default()
            },
            Formula::Sequence(vec![]),
        ),
    ] {
        let formula = Formula::Sequence(vec![Formula::Styled {
            style,
            content: Box::new(content),
        }]);
        assert_eq!(
            write(&formula).unwrap_err().kind,
            WriteErrorKind::InvalidStructure
        );
    }
}
