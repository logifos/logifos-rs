use logifos_model::{Formula, MAX_DEPTH};
use logifos_syntax_mathml::{
    parse, transform, write, TransformErrorKind, WriteErrorKind, MATHML_NAMESPACE,
};

fn symbol(text: &str) -> Formula {
    Formula::Symbol(text.into())
}

fn formula(source: &str) -> Formula {
    transform(&parse(source).unwrap()).unwrap()
}

fn roundtrip(source: &str, expected: Formula, canonical_body: &str) {
    let document = parse(source).unwrap();
    let original = document.clone();
    assert_eq!(transform(&document).unwrap(), expected);
    assert_eq!(document, original);
    let output = write(&expected).unwrap();
    assert_eq!(
        output,
        format!("<math xmlns=\"{MATHML_NAMESPACE}\">{canonical_body}</math>")
    );
    assert_eq!(formula(&output), expected);
    assert_eq!(write(&formula(&output)).unwrap(), output);
}

#[test]
fn under_over_preserve_distinct_placement_and_operand_order() {
    let base = Box::new(symbol("base"));
    let under = Box::new(symbol("under"));
    let over = Box::new(symbol("over"));
    for (body, expected) in [
        (
            "<munder><mi>base</mi><mi>under</mi></munder>",
            Formula::Under(base.clone(), under.clone()),
        ),
        (
            "<mover><mi>base</mi><mi>over</mi></mover>",
            Formula::Over(base.clone(), over.clone()),
        ),
        (
            "<munderover><mi>base</mi><mi>under</mi><mi>over</mi></munderover>",
            Formula::UnderOver(base, under, over),
        ),
    ] {
        roundtrip(
            &format!("<math>{body}</math>"),
            Formula::Sequence(vec![expected]),
            body,
        );
    }
}

#[test]
fn nested_limits_preserve_empty_singleton_and_multi_child_rows() {
    let body = "<munderover><mover><mrow></mrow><mi>top</mi></mover><mrow><mi>i</mi></mrow><mrow><mi>a</mi><mo>+</mo><mi>b</mi></mrow></munderover>";
    roundtrip(
        &format!("<math>{body}</math>"),
        Formula::Sequence(vec![Formula::UnderOver(
            Box::new(Formula::Over(
                Box::new(Formula::Sequence(vec![])),
                Box::new(symbol("top")),
            )),
            Box::new(Formula::Sequence(vec![symbol("i")])),
            Box::new(Formula::Sequence(vec![
                symbol("a"),
                Formula::Operator("+".into()),
                symbol("b"),
            ])),
        )]),
        body,
    );
}

#[test]
fn namespaces_indentation_and_escaped_token_text_roundtrip() {
    roundtrip(
        &format!("<m:math xmlns:m='{MATHML_NAMESPACE}'>\n<m:munder>\t<m:mi>α&amp;&#xD;</m:mi>\n<m:mi><![CDATA[<&]]></m:mi> </m:munder>\n</m:math>"),
        Formula::Sequence(vec![Formula::Under(
            Box::new(symbol("α&\r")),
            Box::new(symbol("<&")),
        )]),
        "<munder><mi>α&amp;&#xD;</mi><mi>&lt;&amp;</mi></munder>",
    );
}

#[test]
fn other_arities_use_core_row_semantics_without_losing_children() {
    for (name, arity) in [("munder", 2), ("mover", 2), ("munderover", 3)] {
        for count in (0..=4).filter(|count| *count != arity) {
            let children: String = (0..count).map(|i| format!("<mi>{i}</mi>")).collect();
            roundtrip(
                &format!("<math><{name}>{children}</{name}></math>"),
                Formula::Sequence(vec![Formula::Sequence(
                    (0..count).map(|i| symbol(&i.to_string())).collect(),
                )]),
                &format!("<mrow>{children}</mrow>"),
            );
        }
    }
}

#[test]
fn presentation_attributes_still_refuse_at_their_owner() {
    for name in ["munder", "mover", "munderover"] {
        for attribute in ["accent", "accentunder", "displaystyle", "mathcolor"] {
            let error =
                transform(&parse(format!("<math><{name} {attribute}='false'/></math>")).unwrap())
                    .unwrap_err();
            assert_eq!(error.kind, TransformErrorKind::UnsupportedAttribute);
            assert_eq!(error.element.local, name);
            assert_eq!(error.attribute.unwrap().local, attribute);
            assert_eq!(error.location, "/root/children/0/attributes/0");
        }
    }
}

#[test]
fn unsupported_content_refuses_even_at_row_fallback_arities() {
    for name in ["munder", "mover", "munderover"] {
        for (content, kind) in [
            ("<!--note-->", TransformErrorKind::UnsupportedContent),
            ("lost", TransformErrorKind::UnsupportedContent),
            ("&#xA0;", TransformErrorKind::UnsupportedContent),
            (
                "<f:mi xmlns:f='urn:foreign'>x</f:mi>",
                TransformErrorKind::UnsupportedElement,
            ),
        ] {
            let error = transform(
                &parse(format!("<math><{name}><mi>x</mi>{content}</{name}></math>")).unwrap(),
            )
            .unwrap_err();
            assert_eq!(error.kind, kind);
            assert_eq!(error.location, "/root/children/0/children/1");
            assert_eq!(error.code(), "UNSUPPORTED_SYNTAX");
        }
    }
}

#[test]
fn foreign_under_over_elements_refuse_despite_known_local_names() {
    for name in ["munder", "mover", "munderover"] {
        let error =
            transform(&parse(format!("<math><f:{name} xmlns:f='urn:foreign'/></math>")).unwrap())
                .unwrap_err();
        assert_eq!(error.kind, TransformErrorKind::UnsupportedElement);
        assert_eq!(error.element.namespace.as_deref(), Some("urn:foreign"));
        assert_eq!(error.location, "/root/children/0");
    }
}

#[test]
fn writer_refusals_name_each_under_over_operand() {
    let valid = Box::new(symbol("x"));
    let invalid = Box::new(Formula::Operator("private".into()));
    for (node, field) in [
        (Formula::Under(invalid.clone(), valid.clone()), "base"),
        (Formula::Under(valid.clone(), invalid.clone()), "under"),
        (Formula::Over(invalid.clone(), valid.clone()), "base"),
        (Formula::Over(valid.clone(), invalid.clone()), "over"),
        (
            Formula::UnderOver(invalid.clone(), valid.clone(), valid.clone()),
            "base",
        ),
        (
            Formula::UnderOver(valid.clone(), invalid.clone(), valid.clone()),
            "under",
        ),
        (Formula::UnderOver(valid.clone(), valid, invalid), "over"),
    ] {
        let error = write(&Formula::Sequence(vec![node.clone()])).unwrap_err();
        assert_eq!(error.kind, WriteErrorKind::UnsupportedOperator);
        assert_eq!(error.location, format!("/children/0/{field}"));
        assert!(!error.to_string().contains("private"));
        assert_eq!(
            write(&node).unwrap_err().kind,
            WriteErrorKind::InvalidStructure
        );
    }
}

#[test]
fn writer_checks_characters_in_nested_limit_operands() {
    let error = write(&Formula::Sequence(vec![Formula::UnderOver(
        Box::new(symbol("x")),
        Box::new(symbol("i")),
        Box::new(symbol("private\0")),
    )]))
    .unwrap_err();
    assert_eq!(error.kind, WriteErrorKind::InvalidCharacter);
    assert_eq!(error.location, "/children/0/over");
    assert!(!error.to_string().contains("private"));
}

#[test]
fn limit_operands_obey_the_shared_depth_budget() {
    let node = Formula::UnderOver(
        Box::new(symbol("x")),
        Box::new(symbol("")),
        Box::new(symbol("")),
    );
    // Reserve the root, limit element, symbol and its nonempty text node.
    let at_limit = (0..MAX_DEPTH - 4).fold(node, |child, _| {
        Formula::Over(Box::new(child), Box::new(symbol("")))
    });
    let formula = Formula::Sequence(vec![at_limit]);
    let output = write(&formula).unwrap();
    assert_eq!(transform(&parse(output).unwrap()).unwrap(), formula);
    let too_deep = Formula::Sequence(vec![formula]);
    assert_eq!(
        write(&too_deep).unwrap_err().kind,
        WriteErrorKind::ResourceLimit
    );
}
