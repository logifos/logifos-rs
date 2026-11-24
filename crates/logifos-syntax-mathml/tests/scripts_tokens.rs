use logifos_model::{Formula, ScriptPair, MAX_DEPTH, MAX_NODES};
use logifos_syntax_mathml::{
    parse, transform, write, MathmlDocument, Node, TransformErrorKind, WriteErrorKind,
    MATHML_NAMESPACE,
};

fn symbol(text: &str) -> Formula {
    Formula::Symbol(text.into())
}

fn pair(subscript: Option<Formula>, superscript: Option<Formula>) -> ScriptPair {
    ScriptPair {
        subscript: subscript.map(Box::new),
        superscript: superscript.map(Box::new),
    }
}

fn multi(postscripts: Vec<ScriptPair>, prescripts: Option<Vec<ScriptPair>>) -> Formula {
    Formula::MultiScripts {
        base: Box::new(symbol("x")),
        postscripts,
        prescripts,
    }
}

fn roundtrip(body: &str, node: Formula, canonical: &str) {
    let formula = Formula::Sequence(vec![node]);
    assert_eq!(
        transform(&parse(format!("<math>{body}</math>")).unwrap()).unwrap(),
        formula
    );
    let output = write(&formula).unwrap();
    assert_eq!(
        output,
        format!("<math xmlns=\"{MATHML_NAMESPACE}\">{canonical}</math>")
    );
    assert_eq!(transform(&parse(&output).unwrap()).unwrap(), formula);
    assert_eq!(
        write(&transform(&parse(output).unwrap()).unwrap()).unwrap(),
        write(&formula).unwrap()
    );
}

fn refused(body: &str, kind: TransformErrorKind, path: &str) {
    let error = transform(&parse(format!("<math>{body}</math>")).unwrap()).unwrap_err();
    assert_eq!(error.kind, kind, "{body}");
    assert_eq!(error.location, path, "{body}");
    assert_eq!(error.code(), "UNSUPPORTED_SYNTAX");
}

#[test]
fn tables_preserve_combined_scripts_multiscripts_and_distinct_tokens() {
    let body = "<mtable><mtr><mtd><msubsup><mi>x</mi><ms>sub</ms><mspace></mspace></msubsup></mtd><mtd><mmultiscripts><ms>base</ms><none></none><mi>post</mi><mprescripts></mprescripts><mspace></mspace><none></none></mmultiscripts></mtd></mtr></mtable>";
    roundtrip(
        body,
        Formula::Table(vec![vec![
            Formula::Sequence(vec![Formula::SubSup(
                Box::new(symbol("x")),
                Box::new(Formula::StringLiteral("sub".into())),
                Box::new(Formula::Space),
            )]),
            Formula::Sequence(vec![Formula::MultiScripts {
                base: Box::new(Formula::StringLiteral("base".into())),
                postscripts: vec![pair(None, Some(symbol("post")))],
                prescripts: Some(vec![pair(Some(Formula::Space), None)]),
            }]),
        ]]),
        body,
    );
}

#[test]
fn tables_remain_distinct_in_combined_and_multiscript_operands() {
    let table = "<mtable><mtr><mtd><mi>x</mi></mtd></mtr></mtable>";
    let node = Formula::Table(vec![vec![Formula::Sequence(vec![symbol("x")])]]);
    let body = format!("<msubsup>{table}<mi>sub</mi><mmultiscripts><mi>x</mi>{table}<none></none></mmultiscripts></msubsup>");
    roundtrip(
        &body,
        Formula::SubSup(
            Box::new(node.clone()),
            Box::new(symbol("sub")),
            Box::new(multi(vec![pair(Some(node), None)], None)),
        ),
        &body,
    );
}

#[test]
fn combined_scripts_preserve_all_three_operands() {
    let body = "<msubsup><mi>x</mi><mrow></mrow><mrow><mi>a</mi><mi>b</mi></mrow></msubsup>";
    roundtrip(
        body,
        Formula::SubSup(
            Box::new(symbol("x")),
            Box::new(Formula::Sequence(vec![])),
            Box::new(Formula::Sequence(vec![symbol("a"), symbol("b")])),
        ),
        body,
    );
}

#[test]
fn combined_scripts_use_rows_at_other_arities() {
    for count in [0, 1, 2, 4] {
        let body: String = (0..count).map(|i| format!("<mi>{i}</mi>")).collect();
        roundtrip(
            &format!("<msubsup>{body}</msubsup>"),
            Formula::Sequence((0..count).map(|i| symbol(&i.to_string())).collect()),
            &format!("<mrow>{body}</mrow>"),
        );
    }
}

#[test]
fn multiscripts_preserve_pair_order_and_missing_slots_on_both_sides() {
    let body = "<mmultiscripts><mi>x</mi><mi>a</mi><none/><mrow></mrow><mi>b</mi><mprescripts/><none/><mi>c</mi><mi>d</mi><none/></mmultiscripts>";
    let canonical = body
        .replace("<none/>", "<none></none>")
        .replace("<mprescripts/>", "<mprescripts></mprescripts>");
    roundtrip(
        body,
        multi(
            vec![
                pair(Some(symbol("a")), None),
                pair(Some(Formula::Sequence(vec![])), Some(symbol("b"))),
            ],
            Some(vec![
                pair(None, Some(symbol("c"))),
                pair(Some(symbol("d")), None),
            ]),
        ),
        &canonical,
    );
}

#[test]
fn empty_and_absent_prescript_boundaries_remain_distinct() {
    for (suffix, prescripts) in [("", None), ("<mprescripts></mprescripts>", Some(vec![]))] {
        let body = format!("<mmultiscripts><mi>x</mi>{suffix}</mmultiscripts>");
        roundtrip(&body, multi(vec![], prescripts), &body);
    }
    let body = "<mmultiscripts><mi>x</mi><none/><none/></mmultiscripts>";
    roundtrip(
        body,
        multi(vec![pair(None, None)], None),
        &body.replace("<none/>", "<none></none>"),
    );
}

#[test]
fn nested_multiscripts_and_combined_scripts_preserve_rows() {
    let body = "<mmultiscripts><msubsup><mrow><mi>x</mi></mrow><mi>a</mi><mi>b</mi></msubsup><mprescripts/><mmultiscripts><mi>x</mi></mmultiscripts><mrow></mrow></mmultiscripts>";
    roundtrip(
        body,
        Formula::MultiScripts {
            base: Box::new(Formula::SubSup(
                Box::new(Formula::Sequence(vec![symbol("x")])),
                Box::new(symbol("a")),
                Box::new(symbol("b")),
            )),
            postscripts: vec![],
            prescripts: Some(vec![pair(
                Some(multi(vec![], None)),
                Some(Formula::Sequence(vec![])),
            )]),
        },
        &body.replace("<mprescripts/>", "<mprescripts></mprescripts>"),
    );
}

#[test]
fn namespaced_indented_scripts_and_markers_roundtrip() {
    let source = format!("<m:math xmlns:m='{MATHML_NAMESPACE}'><m:mmultiscripts>\n<m:mi>x</m:mi> <m:none>\t</m:none><m:ms><![CDATA[<&]]>&#xD;α</m:ms> <m:mprescripts> </m:mprescripts></m:mmultiscripts></m:math>");
    let formula = Formula::Sequence(vec![multi(
        vec![pair(None, Some(Formula::StringLiteral("<&\rα".into())))],
        Some(vec![]),
    )]);
    assert_eq!(transform(&parse(source).unwrap()).unwrap(), formula);
    let output = write(&formula).unwrap();
    assert!(output.contains("<ms>&lt;&amp;&#xD;α</ms>"));
    assert_eq!(transform(&parse(output).unwrap()).unwrap(), formula);
}

#[test]
fn strings_and_default_spaces_keep_their_distinct_kinds() {
    for (body, text, canonical) in [
        ("<ms/>", "", "<ms></ms>"),
        (
            "<ms> \tα&amp;&lt;&#xD;\" </ms>",
            " \tα&<\r\" ",
            "<ms> \tα&amp;&lt;&#xD;\" </ms>",
        ),
    ] {
        roundtrip(body, Formula::StringLiteral(text.into()), canonical);
    }
    roundtrip("<mspace>\n\t</mspace>", Formula::Space, "<mspace></mspace>");
}

#[test]
fn dimensions_quotes_and_presentation_attributes_refuse() {
    for (name, attribute) in [
        ("mspace", "width"),
        ("mspace", "height"),
        ("mspace", "depth"),
        ("ms", "lquote"),
        ("ms", "rquote"),
        ("msubsup", "subscriptshift"),
        ("mmultiscripts", "mathcolor"),
    ] {
        refused(
            &format!("<{name} {attribute}='private'/>"),
            TransformErrorKind::UnsupportedAttribute,
            "/root/children/0/attributes/0",
        );
    }
    for name in ["none", "mprescripts"] {
        refused(
            &format!("<mmultiscripts><mi>x</mi><{name} mathcolor='private'/></mmultiscripts>"),
            TransformErrorKind::UnsupportedAttribute,
            "/root/children/0/children/1/attributes/0",
        );
    }
}

#[test]
fn malformed_multiscripts_refuse_without_dropping_operands() {
    for body in [
        "",
        "<mi>x</mi><mi>a</mi>",
        "<mi>x</mi><mprescripts/><mi>a</mi>",
    ] {
        refused(
            &format!("<mmultiscripts>{body}</mmultiscripts>"),
            TransformErrorKind::InvalidStructure,
            "/root/children/0",
        );
    }
    for (body, index) in [
        ("<mprescripts/>", 0),
        ("<none/>", 0),
        ("<mi>x</mi><mi>a</mi><mprescripts/>", 2),
        ("<mi>x</mi><mprescripts/><mprescripts/>", 2),
    ] {
        refused(
            &format!("<mmultiscripts>{body}</mmultiscripts>"),
            TransformErrorKind::InvalidStructure,
            &format!("/root/children/0/children/{index}"),
        );
    }
}

#[test]
fn markers_and_empty_spaces_never_discard_content() {
    for name in ["none", "mprescripts"] {
        for (content, kind) in [
            ("private", TransformErrorKind::UnsupportedContent),
            ("<!--private-->", TransformErrorKind::UnsupportedContent),
            ("&#xA0;", TransformErrorKind::UnsupportedContent),
            ("<mi>x</mi>", TransformErrorKind::InvalidStructure),
        ] {
            refused(
                &format!("<mmultiscripts><mi>x</mi><{name}>{content}</{name}></mmultiscripts>"),
                kind,
                "/root/children/0/children/1/children/0",
            );
        }
    }
    for (content, kind) in [
        ("private", TransformErrorKind::UnsupportedContent),
        ("<!--private-->", TransformErrorKind::UnsupportedContent),
        ("<mi>x</mi>", TransformErrorKind::InvalidStructure),
    ] {
        refused(
            &format!("<mspace>{content}</mspace>"),
            kind,
            "/root/children/0/children/0",
        );
        if content != "private" {
            refused(
                &format!("<ms>{content}</ms>"),
                kind,
                "/root/children/0/children/0",
            );
        }
    }
}

#[test]
fn foreign_elements_and_unsupported_operands_still_refuse() {
    for name in [
        "msubsup",
        "mmultiscripts",
        "mprescripts",
        "none",
        "mspace",
        "ms",
    ] {
        refused(
            &format!("<f:{name} xmlns:f='urn:foreign'/>"),
            TransformErrorKind::UnsupportedElement,
            "/root/children/0",
        );
        if matches!(name, "mprescripts" | "none") {
            refused(
                &format!(
                    "<mmultiscripts><mi>x</mi><f:{name} xmlns:f='urn:foreign'/></mmultiscripts>"
                ),
                TransformErrorKind::UnsupportedElement,
                "/root/children/0/children/1",
            );
        }
    }
    for content in [
        "<mpadded/>",
        "<mo>private</mo>",
        "<!--private-->",
        "private",
    ] {
        for name in ["msubsup", "mmultiscripts"] {
            let error = transform(
                &parse(format!("<math><{name}><mi>x</mi>{content}</{name}></math>")).unwrap(),
            )
            .unwrap_err();
            assert_eq!(error.location, "/root/children/0/children/1");
            assert_eq!(error.code(), "UNSUPPORTED_SYNTAX");
        }
    }
}

#[test]
fn writer_refusals_name_each_script_operand() {
    let valid = Box::new(symbol("x"));
    let invalid = Box::new(Formula::Operator("private".into()));
    let nodes = [
        (
            Formula::SubSup(invalid.clone(), valid.clone(), valid.clone()),
            "base",
        ),
        (
            Formula::SubSup(valid.clone(), invalid.clone(), valid.clone()),
            "subscript",
        ),
        (
            Formula::SubSup(valid.clone(), valid, invalid.clone()),
            "superscript",
        ),
        (
            Formula::MultiScripts {
                base: invalid.clone(),
                postscripts: vec![],
                prescripts: None,
            },
            "base",
        ),
        (
            multi(vec![pair(Some(*invalid.clone()), None)], None),
            "postscripts/0/subscript",
        ),
        (
            multi(vec![pair(None, Some(*invalid.clone()))], None),
            "postscripts/0/superscript",
        ),
        (
            multi(vec![], Some(vec![pair(Some(*invalid.clone()), None)])),
            "prescripts/0/subscript",
        ),
        (
            multi(vec![], Some(vec![pair(None, Some(*invalid))])),
            "prescripts/0/superscript",
        ),
    ];
    for (node, field) in nodes {
        let error = write(&Formula::Sequence(vec![node])).unwrap_err();
        assert_eq!(error.kind, WriteErrorKind::UnsupportedOperator);
        assert_eq!(error.location, format!("/children/0/{field}"));
        assert!(!error.to_string().contains("private"));
    }
    let error = write(&Formula::Sequence(vec![Formula::StringLiteral(
        "private\0".into(),
    )]))
    .unwrap_err();
    assert_eq!(error.kind, WriteErrorKind::InvalidCharacter);
    assert_eq!(error.location, "/children/0");
    assert!(!error.to_string().contains("private"));
}

#[test]
fn script_and_marker_depth_limits_roundtrip_at_the_boundary() {
    let node = multi(vec![pair(None, None)], Some(vec![]));
    let at_limit = (0..MAX_DEPTH - 4).fold(node, |base, _| {
        Formula::SubSup(Box::new(base), Box::new(symbol("")), Box::new(symbol("")))
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

#[test]
fn multiscript_markers_count_toward_node_limits() {
    let mut document =
        parse("<math><mmultiscripts><mi>x</mi><none/><none/></mmultiscripts></math>").unwrap();
    let Node::Element(multiscripts) = &mut document.root.children[0] else {
        unreachable!()
    };
    let marker = multiscripts.children[1].clone();
    multiscripts
        .children
        .extend(std::iter::repeat_n(marker, MAX_NODES));
    assert_eq!(
        transform(&document).unwrap_err().kind,
        TransformErrorKind::ResourceLimit
    );
    let formula = Formula::Sequence(vec![multi(vec![pair(None, None); MAX_NODES / 2], None)]);
    assert_eq!(
        write(&formula).unwrap_err().kind,
        WriteErrorKind::ResourceLimit
    );
}

#[test]
fn manually_built_syntax_also_checks_marker_depth() {
    let mut document: MathmlDocument =
        parse("<math><mmultiscripts><mi/><mprescripts/></mmultiscripts></math>").unwrap();
    let row = parse("<math><mrow/></math>")
        .unwrap()
        .root
        .children
        .remove(0);
    for _ in 0..MAX_DEPTH {
        let mut wrapper = row.clone();
        let Node::Element(element) = &mut wrapper else {
            unreachable!()
        };
        element.children = std::mem::take(&mut document.root.children);
        document.root.children = vec![wrapper];
    }
    assert_eq!(
        transform(&document).unwrap_err().kind,
        TransformErrorKind::ResourceLimit
    );
}
