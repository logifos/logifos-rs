use logifos_model::{Formula, MAX_DEPTH, MAX_NODES};
use logifos_syntax_mathml::{
    parse, parse_with_limits, transform, write, ParseLimits, TransformErrorKind, WriteErrorKind,
    MATHML_NAMESPACE,
};

fn symbol(text: &str) -> Formula {
    Formula::Symbol(text.into())
}

fn roundtrip(formula: &Formula) -> String {
    let original = formula.clone();
    let output = write(formula).unwrap();
    assert_eq!(transform(&parse(&output).unwrap()).unwrap(), *formula);
    assert_eq!(*formula, original, "writer mutated its input");
    assert_eq!(
        write(formula).unwrap(),
        output,
        "writer is nondeterministic"
    );
    output
}

#[test]
fn fresh_constructs_roundtrip_with_exact_formula_equality() {
    let cases = [
        "<math/>",
        "<math><mi/></math>",
        "<math><mn>007.00</mn><mtext> units </mtext><mi>α</mi></math>",
        "<math><mo>+</mo><mo>-</mo><mo>=</mo><mo>*</mo><mo>×</mo></math>",
        "<math><mrow/><mrow><mi>x</mi></mrow><mrow><mrow/></mrow></math>",
        "<math><mfrac><mi>top</mi><mi>bottom</mi></mfrac></math>",
        "<math><msub><mi>base</mi><mi>sub</mi></msub></math>",
        "<math><msup><mi>base</mi><mi>sup</mi></msup></math>",
        "<math><msub><mfrac><mi>a</mi><mi>b</mi></mfrac><msup><mi>c</mi><mi>d</mi></msup></msub></math>",
        "<math><mfrac><mrow/><mrow><mi>a</mi><mi>b</mi></mrow></mfrac></math>",
        "<math><msqrt><mi>x</mi><mo>+</mo><mn>1</mn></msqrt></math>",
        "<math><msqrt/></math>",
        "<math><mroot><msqrt><mi>x</mi></msqrt><mn>3</mn></mroot></math>",
        "<math><msqrt><mfrac><mi>a</mi><mi>b</mi></mfrac></msqrt><mroot><mi>r</mi><mi>n</mi></mroot></math>",
        "<math><mtable><mtr><mtd><mn>1</mn></mtd><mtd><mn>2</mn></mtd></mtr><mtr><mtd><mi>a</mi></mtd></mtr></mtable></math>",
        "<math><mtable><mtr><mtd><mtable><mtr><mtd><mi>n</mi></mtd></mtr></mtable></mtd></mtr></mtable></math>",
        "<math><mo>(</mo><mi>x</mi><mo>)</mo></math>",
        "<math><mo>\u{2308}</mo><mfrac><mi>a</mi><mi>b</mi></mfrac><mo>\u{2309}</mo></math>",
        "<math><mtext> &lt;&amp;&gt;\"' &#xD;\n\tα&#x301;&#x1D6FC;&#xA0; </mtext></math>",
        "<math><mi>a<![CDATA[<&]]>b&amp;lt;</mi></math>",
        "<math>\r\n<mrow>\t<mn>1</mn>\n<mi>x</mi></mrow> </math>",
        "<m:math xmlns:m='http://www.w3.org/1998/Math/MathML'><m:mi>q</m:mi></m:math>",
        "<math xmlns='http://www.w3.org/1998/Math/MathML'><mrow xmlns=''><mi>x</mi></mrow></math>",
    ];
    for source in cases {
        roundtrip(&transform(&parse(source).unwrap()).unwrap());
    }
    println!(
        "fresh: total={}, round-tripped={}, typed-refusals=0",
        cases.len(),
        cases.len()
    );
}

#[test]
fn deterministic_serialization_choices_are_explicit() {
    let formula = Formula::Sequence(vec![symbol("<&>\r\n\t\"'"), Formula::Sequence(vec![])]);
    assert_eq!(
        roundtrip(&formula),
        format!("<math xmlns=\"{MATHML_NAMESPACE}\"><mi>&lt;&amp;&gt;&#xD;\n\t\"'</mi><mrow></mrow></math>")
    );
    assert_eq!(
        roundtrip(&Formula::Sequence(vec![])),
        format!("<math xmlns=\"{MATHML_NAMESPACE}\"></math>")
    );
}

#[test]
fn markup_looking_text_and_cdata_end_are_inert() {
    roundtrip(&Formula::Sequence(vec![symbol(
        "</mi><mo>+</mo><!--note-->]]>&amp;lt;&#13;",
    )]));
}

#[test]
fn every_non_sequence_root_refuses_instead_of_adding_grouping() {
    for formula in [
        symbol("x"),
        Formula::Operator("+".into()),
        Formula::Fraction(Box::new(symbol("a")), Box::new(symbol("b"))),
        Formula::Subscript(Box::new(symbol("a")), Box::new(symbol("b"))),
        Formula::Superscript(Box::new(symbol("a")), Box::new(symbol("b"))),
    ] {
        let error = write(&formula).unwrap_err();
        assert_eq!(error.kind, WriteErrorKind::InvalidStructure);
        assert_eq!(error.location, "");
        assert_eq!(error.code(), "UNSUPPORTED_SYNTAX");
    }
}

#[test]
fn only_sequence_radicands_roundtrip_through_msqrt() {
    // A bare non-sequence radicand has no spelling that reparses equal, so
    // it refuses at the radicand path instead of normalizing silently.
    let error = write(&Formula::Sequence(vec![Formula::Sqrt(Box::new(symbol(
        "x",
    )))]))
    .unwrap_err();
    assert_eq!(error.kind, WriteErrorKind::InvalidStructure);
    assert_eq!(error.location, "/children/0/radicand");
    let output = roundtrip(&Formula::Sequence(vec![Formula::Root(
        Box::new(symbol("x")),
        Box::new(symbol("3")),
    )]));
    assert!(
        output.contains("<mroot><mi>x</mi><mi>3</mi></mroot>"),
        "root bytes: {output}"
    );
}

#[test]
fn unsupported_operators_refuse_at_the_exact_formula_path() {
    for text in ["", "−", "÷", "∑", " + ", "++", "\u{2062}"] {
        let formula = Formula::Sequence(vec![Formula::Fraction(
            Box::new(symbol("x")),
            Box::new(Formula::Superscript(
                Box::new(symbol("y")),
                Box::new(Formula::Operator(text.into())),
            )),
        )]);
        let error = write(&formula).unwrap_err();
        assert_eq!(error.kind, WriteErrorKind::UnsupportedOperator);
        assert_eq!(error.location, "/children/0/denominator/script");
        assert_eq!(error.code(), "UNSUPPORTED_SYNTAX");
    }
}

#[test]
fn invalid_xml_characters_refuse_without_echoing_text() {
    for ch in [
        '\0', '\u{1}', '\u{8}', '\u{b}', '\u{c}', '\u{1f}', '\u{fffe}', '\u{ffff}',
    ] {
        let error = write(&Formula::Sequence(vec![symbol(&format!("private{ch}"))])).unwrap_err();
        assert_eq!(error.kind, WriteErrorKind::InvalidCharacter);
        assert_eq!(error.location, "/children/0");
        assert!(!error.to_string().contains("private"));
        assert!(std::error::Error::source(&error).is_none());
    }
    roundtrip(&Formula::Sequence(vec![symbol(
        "\u{7f}\u{85}\u{fffd}\u{10000}\u{10ffff}",
    )]));
}

fn nested_rows(depth: usize, leaf: Formula) -> Formula {
    (1..depth).fold(leaf, |child, _| Formula::Sequence(vec![child]))
}

#[test]
fn syntax_depth_limit_counts_nonempty_token_text() {
    roundtrip(&nested_rows(MAX_DEPTH, Formula::Sequence(vec![])));
    roundtrip(&nested_rows(MAX_DEPTH, symbol("")));
    roundtrip(&nested_rows(MAX_DEPTH - 1, symbol("x")));
    for formula in [
        nested_rows(MAX_DEPTH + 1, Formula::Sequence(vec![])),
        nested_rows(MAX_DEPTH, symbol("x")),
    ] {
        let error = write(&formula).unwrap_err();
        assert_eq!(error.kind, WriteErrorKind::ResourceLimit);
        assert_eq!(error.code(), "RESOURCE_LIMIT");
    }
}

#[test]
fn syntax_node_limit_counts_empty_tokens_and_text() {
    let mut formula = Formula::Sequence(vec![symbol(""); MAX_NODES - 1]);
    roundtrip(&formula);
    let Formula::Sequence(children) = &mut formula else {
        unreachable!()
    };
    children.push(symbol(""));
    assert_eq!(
        write(&formula).unwrap_err().kind,
        WriteErrorKind::ResourceLimit
    );

    let mut formula = Formula::Sequence(vec![symbol("x"); (MAX_NODES - 1) / 2]);
    let Formula::Sequence(children) = &mut formula else {
        unreachable!()
    };
    children.push(symbol(""));
    roundtrip(&formula);
    let Formula::Sequence(children) = &mut formula else {
        unreachable!()
    };
    children.last_mut().unwrap().clone_from(&symbol("x"));
    assert_eq!(
        write(&formula).unwrap_err().kind,
        WriteErrorKind::ResourceLimit
    );
}

#[test]
fn output_byte_limit_counts_escaping_and_utf8() {
    let overhead = write(&Formula::Sequence(vec![symbol("")])).unwrap().len();
    let available = ParseLimits::default().source_bytes - overhead;
    for (text, serialized_width) in [("a", 1), ("α", 2), ("&", 5), ("\r", 5)] {
        let count = available / serialized_width;
        let padding = available % serialized_width;
        let value = text.repeat(count) + &"z".repeat(padding);
        let formula = Formula::Sequence(vec![symbol(&value)]);
        assert_eq!(
            roundtrip(&formula).len(),
            ParseLimits::default().source_bytes
        );
        let error = write(&Formula::Sequence(vec![symbol(&(value + "a"))])).unwrap_err();
        assert_eq!(error.kind, WriteErrorKind::ResourceLimit);
    }
    for formula in [
        Formula::Sequence(vec![symbol(
            &"x".repeat(ParseLimits::default().source_bytes + 1),
        )]),
        Formula::Sequence(vec![
            symbol(&"x".repeat(600_000)),
            symbol(&"y".repeat(600_000)),
        ]),
    ] {
        assert_eq!(
            write(&formula).unwrap_err().kind,
            WriteErrorKind::ResourceLimit
        );
    }
}

#[test]
fn attributes_and_inherited_style_are_typed_refusals_never_dropped() {
    for attribute in [
        "mathcolor='chartreuse'",
        "mathbackground='white'",
        "mathvariant='unknown'",
        "mathsize='12pt'",
        "dir='rtl'",
        "displaystyle='invalid'",
        "scriptlevel='256'",
        "display='invalid'",
        "xml:lang='el'",
        "class='formula'",
        "id='equation'",
    ] {
        for source in [
            format!("<math {attribute}><mrow><mi>x</mi></mrow></math>"),
            format!("<math><mrow {attribute}><mi>x</mi></mrow></math>"),
            format!("<math><mi {attribute}>x</mi></math>"),
        ] {
            let error = transform(&parse(source).unwrap()).unwrap_err();
            assert_eq!(error.kind, TransformErrorKind::UnsupportedAttribute);
            assert!(error.attribute.is_some());
        }
    }
    let error =
        transform(&parse("<math><menclose><mi>x</mi></menclose></math>").unwrap()).unwrap_err();
    assert_eq!(error.kind, TransformErrorKind::UnsupportedElement);
}

#[test]
fn existing_corpora_roundtrip_or_refuse_typed() {
    let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (manifest_path, expected_counts) in [
        (crate_root.join("tests/cases.json"), (8, 2, 6)),
        (
            crate_root.join("../../corpus/conformance/manifest.json"),
            (3, 2, 1),
        ),
    ] {
        let manifest = logifos_json::decode(&std::fs::read(&manifest_path).unwrap()).unwrap();
        let cases = manifest
            .as_array()
            .unwrap_or_else(|| manifest["cases"].as_array().unwrap());
        let (mut total, mut passed, mut refused) = (0, 0, 0);
        for case in cases.iter().filter(|c| c["input_kind"] == "mathml") {
            let id = case["id"].as_str().unwrap();
            let source = std::fs::read(
                manifest_path
                    .parent()
                    .unwrap()
                    .join(case["path"].as_str().unwrap()),
            )
            .unwrap();
            let assertion = &case["expected"]["assertion"];
            let hard = ParseLimits::default();
            let limits = ParseLimits {
                source_bytes: assertion["source_bytes"]
                    .as_u64()
                    .map_or(hard.source_bytes, |n| usize::try_from(n).unwrap()),
                tree_depth: assertion["tree_depth"]
                    .as_u64()
                    .map_or(hard.tree_depth, |n| usize::try_from(n).unwrap()),
                ..hard
            };
            total += 1;
            match parse_with_limits(source, limits) {
                Err(error) => {
                    assert_eq!(case["expected"]["kind"], "refusal", "{id}");
                    assert_eq!(
                        format!("{:?}", error.kind),
                        assertion["error_kind"].as_str().unwrap(),
                        "{id}"
                    );
                    refused += 1;
                }
                Ok(document) => match transform(&document) {
                    Ok(formula) => {
                        assert!(
                            matches!(id, "mathml-namespace-token" | "mathml-prefixed-token"),
                            "unexpectedly supported: {id}"
                        );
                        roundtrip(&formula);
                        passed += 1;
                    }
                    Err(error) => {
                        let expected = match id {
                            "mathml-inherited-style" => TransformErrorKind::UnsupportedAttribute,
                            "mathml-mixed-content" => TransformErrorKind::InvalidStructure,
                            _ => panic!("unexpected transform refusal: {id}: {error}"),
                        };
                        assert_eq!(error.kind, expected, "{id}");
                        refused += 1;
                    }
                },
            }
        }
        assert_eq!((total, passed, refused), expected_counts);
        println!(
            "{}: total={total}, round-tripped={passed}, typed-refusals={refused}",
            manifest_path.display()
        );
    }
}
