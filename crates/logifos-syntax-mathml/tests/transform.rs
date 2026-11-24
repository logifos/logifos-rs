use logifos_model::{Formula, MAX_DEPTH, MAX_NODES};
use logifos_syntax_mathml::{
    parse, transform, MathmlDocument, Node, TransformError, TransformErrorKind, MATHML_NAMESPACE,
};

fn converted(source: &str) -> Formula {
    let document = parse(source).unwrap();
    let original = document.clone();
    let formula = transform(&document).unwrap();
    assert_eq!(document, original, "conversion changed syntax");
    let serialized = logifos_syntax_mathml::write(&formula).unwrap();
    assert_eq!(transform(&parse(serialized).unwrap()).unwrap(), formula);
    formula
}

fn symbol(text: &str) -> Formula {
    Formula::Symbol(text.into())
}

fn operator(text: &str) -> Formula {
    Formula::Operator(text.into())
}

fn refused(source: &str, kind: TransformErrorKind, name: &str, location: &str) -> TransformError {
    let document = parse(source).unwrap();
    let error = transform(&document).unwrap_err();
    assert_eq!(error.kind, kind, "{source}: {error}");
    assert_eq!(error.element.local, name, "{source}");
    assert_eq!(error.location, location, "{source}");
    assert_eq!(error.code(), "UNSUPPORTED_SYNTAX");
    assert!(error.to_string().contains(name));
    assert!(error.to_string().contains(location));
    error
}

fn unsupported(elements: &[(&str, &str)]) {
    for (name, markup) in elements {
        refused(
            &format!("<math><mi>before</mi>{markup}<mi>after</mi></math>"),
            TransformErrorKind::UnsupportedElement,
            name,
            "/root/children/1",
        );
    }
}

#[test]
fn implicit_row_retains_token_order_and_symbol_identity() {
    assert_eq!(
        converted("<math><mi>q</mi><mo>+</mo><mn>17</mn><mtext> units </mtext><mi>+</mi></math>"),
        Formula::Sequence(vec![
            symbol("q"),
            Formula::Operator("+".into()),
            symbol("17"),
            symbol(" units "),
            symbol("+"),
        ])
    );
}

#[test]
fn all_five_typed_operators_keep_their_exact_spelling() {
    assert_eq!(
        converted("<math><mo>+</mo><mo>-</mo><mo>=</mo><mo>*</mo><mo>×</mo></math>"),
        Formula::Sequence(vec![
            Formula::Operator("+".into()),
            Formula::Operator("-".into()),
            Formula::Operator("=".into()),
            Formula::Operator("*".into()),
            Formula::Operator("×".into()),
        ])
    );
}

#[test]
fn empty_singleton_and_nested_rows_keep_grouping() {
    assert_eq!(converted("<math/>"), Formula::Sequence(vec![]));
    assert_eq!(
        converted("<math><mrow/><mrow><mi>r</mi><mrow><mn>8</mn></mrow></mrow></math>"),
        Formula::Sequence(vec![
            Formula::Sequence(vec![]),
            Formula::Sequence(vec![symbol("r"), Formula::Sequence(vec![symbol("8")])]),
        ])
    );
}

#[test]
fn fraction_uses_numerator_then_denominator() {
    assert_eq!(
        converted(
            "<math><mfrac>\n<mi>top</mi>\t<mrow><mi>bottom</mi><mn>4</mn></mrow> </mfrac></math>"
        ),
        Formula::Sequence(vec![Formula::Fraction(
            Box::new(symbol("top")),
            Box::new(Formula::Sequence(vec![symbol("bottom"), symbol("4")])),
        )])
    );
}

#[test]
fn separate_scripts_use_base_then_script() {
    assert_eq!(
        converted(
            "<math><msub><mi>a</mi><mn>3</mn></msub><msup><mi>b</mi><mn>5</mn></msup></math>"
        ),
        Formula::Sequence(vec![
            Formula::Subscript(Box::new(symbol("a")), Box::new(symbol("3"))),
            Formula::Superscript(Box::new(symbol("b")), Box::new(symbol("5"))),
        ])
    );
}

#[test]
fn nested_fraction_and_scripts_keep_each_operand() {
    assert_eq!(
        converted("<math><msup><msub><mi>z</mi><mi>j</mi></msub><mfrac><mn>7</mn><msup><mi>t</mi><mn>2</mn></msup></mfrac></msup></math>"),
        Formula::Sequence(vec![Formula::Superscript(
            Box::new(Formula::Subscript(
                Box::new(symbol("z")),
                Box::new(symbol("j")),
            )),
            Box::new(Formula::Fraction(
                Box::new(symbol("7")),
                Box::new(Formula::Superscript(
                    Box::new(symbol("t")),
                    Box::new(symbol("2")),
                )),
            )),
        )])
    );
}

#[test]
fn decoded_text_and_cdata_keep_unicode_and_whitespace() {
    assert_eq!(
        converted("<math>\n<mi> α&#x301; </mi><mn>007.00</mn><mtext>a<![CDATA[<&]]>&amp;&#xA0;b</mtext><mi/></math>"),
        Formula::Sequence(vec![
            symbol(" α\u{301} "),
            symbol("007.00"),
            symbol("a<&&\u{a0}b"),
            symbol(""),
        ])
    );
}

#[test]
fn namespace_prefixes_do_not_change_formula() {
    let expected = Formula::Sequence(vec![symbol("ν")]);
    assert_eq!(converted("<math><mi>ν</mi></math>"), expected);
    assert_eq!(
        converted(&format!(
            "<math xmlns='{MATHML_NAMESPACE}'><mi>ν</mi></math>"
        )),
        expected
    );
    assert_eq!(
        converted(&format!(
            "<m:math xmlns:m='{MATHML_NAMESPACE}' xmlns:e='urn:unused'><m:mi>ν</m:mi></m:math>"
        )),
        expected
    );
}

#[test]
fn radicals_map_the_radicand_and_index() {
    assert_eq!(
        converted("<math><msqrt><mi>x</mi><mo>+</mo><mn>1</mn></msqrt></math>"),
        Formula::Sequence(vec![Formula::Sqrt(Box::new(Formula::Sequence(vec![
            symbol("x"),
            operator("+"),
            symbol("1"),
        ])))])
    );
    assert_eq!(
        converted("<math><msqrt/></math>"),
        Formula::Sequence(vec![Formula::Sqrt(Box::new(Formula::Sequence(vec![])))])
    );
    assert_eq!(
        converted("<math><mroot><mi>x</mi><mn>3</mn></mroot></math>"),
        Formula::Sequence(vec![Formula::Root(
            Box::new(symbol("x")),
            Box::new(symbol("3")),
        )])
    );
}

#[test]
fn mroot_outside_its_arity_becomes_a_row() {
    assert_eq!(
        converted("<math><mroot><mi>x</mi></mroot></math>"),
        Formula::Sequence(vec![Formula::Sequence(vec![symbol("x")])])
    );
    assert_eq!(
        converted("<math><mroot><mi>a</mi><mn>3</mn><mn>4</mn></mroot></math>"),
        Formula::Sequence(vec![Formula::Sequence(vec![
            symbol("a"),
            symbol("3"),
            symbol("4")
        ])])
    );
}

#[test]
fn fences_become_stretchy_operators() {
    unsupported(&[("mfenced", "<mfenced><mi>x</mi></mfenced>")]);
    let expected = |token: &str| Formula::Sequence(vec![Formula::Operator(token.into())]);
    for token in ["(", ")", "[", "]", "{", "}", "|"] {
        assert_eq!(
            converted(&format!("<math><mo>{token}</mo></math>")),
            expected(token)
        );
    }
    for (token, escaped) in [
        ("\u{2016}", "&#x2016;"),
        ("\u{2308}", "&#x2308;"),
        ("\u{2309}", "&#x2309;"),
        ("\u{230a}", "&#x230a;"),
        ("\u{230b}", "&#x230b;"),
        ("\u{27e8}", "&#x27e8;"),
        ("\u{27e9}", "&#x27e9;"),
    ] {
        assert_eq!(
            converted(&format!("<math><mo>{escaped}</mo></math>")),
            expected(token)
        );
    }
}

#[test]
fn tables_map_rows_of_anonymous_row_cells() {
    assert_eq!(
        converted(
            "<math><mtable><mtr><mtd><mn>1</mn></mtd><mtd><mn>2</mn></mtd></mtr><mtr><mtd><mi>a</mi><mo>+</mo></mtd></mtr></mtable></math>"
        ),
        Formula::Sequence(vec![Formula::Table(vec![
            vec![
                Formula::Sequence(vec![symbol("1")]),
                Formula::Sequence(vec![symbol("2")]),
            ],
            vec![Formula::Sequence(vec![symbol("a"), operator("+")])],
        ])])
    );
    assert_eq!(
        converted("<math><mtable/></math>"),
        Formula::Sequence(vec![Formula::Table(vec![])])
    );
}

#[test]
fn loose_table_content_refuses_at_the_offending_element() {
    unsupported(&[
        ("mlabeledtr", "<mlabeledtr/>"),
        ("maligngroup", "<maligngroup/>"),
        ("malignmark", "<malignmark/>"),
    ]);
    refused(
        "<math><mtable><mtd><mi>x</mi></mtd></mtable></math>",
        TransformErrorKind::UnsupportedElement,
        "mtd",
        "/root/children/0/children/0",
    );
    refused(
        "<math><mtable><mtr><mi>x</mi></mtr></mtable></math>",
        TransformErrorKind::UnsupportedElement,
        "mi",
        "/root/children/0/children/0/children/0",
    );
}

#[test]
fn standalone_multiscript_markers_refuse() {
    unsupported(&[("mprescripts", "<mprescripts/>"), ("none", "<none/>")]);
}

#[test]
fn style_wrappers_refuse_even_without_attributes() {
    unsupported(&[("mstyle", "<mstyle><mi>x</mi></mstyle>")]);
}

#[test]
fn spacing_padding_phantoms_and_enclosures_refuse() {
    unsupported(&[
        ("mpadded", "<mpadded><mi>x</mi></mpadded>"),
        ("mphantom", "<mphantom><mi>x</mi></mphantom>"),
        ("menclose", "<menclose><mi>x</mi></menclose>"),
    ]);
}

#[test]
fn semantics_and_annotations_refuse() {
    unsupported(&[
        ("semantics", "<semantics><mi>x</mi></semantics>"),
        ("annotation", "<annotation>meaning</annotation>"),
        (
            "annotation-xml",
            "<annotation-xml><mi>x</mi></annotation-xml>",
        ),
    ]);
}

#[test]
fn actions_errors_and_glyphs_refuse() {
    unsupported(&[
        ("maction", "<maction><mi>x</mi></maction>"),
        ("merror", "<merror><mi>x</mi></merror>"),
        ("mglyph", "<mglyph/>"),
    ]);
}

#[test]
fn content_math_unknown_elements_and_nested_math_refuse() {
    unsupported(&[
        ("apply", "<apply><plus/><ci>x</ci><cn>2</cn></apply>"),
        ("future", "<future><mi>x</mi></future>"),
        ("math", "<math><mi>x</mi></math>"),
    ]);
}

#[test]
fn foreign_namespace_elements_refuse_despite_known_local_name() {
    let error = refused(
        "<math><f:mi xmlns:f='urn:foreign'>x</f:mi></math>",
        TransformErrorKind::UnsupportedElement,
        "mi",
        "/root/children/0",
    );
    assert_eq!(error.element.namespace.as_deref(), Some("urn:foreign"));
    assert_eq!(error.element.prefix.as_deref(), Some("f"));
}

#[test]
fn inherited_presentation_attributes_refuse_at_their_owner() {
    for attribute in [
        "mathcolor='red'",
        "mathbackground='white'",
        "mathvariant='normal'",
        "mathsize='12pt'",
        "dir='ltr'",
        "displaystyle='false'",
        "scriptlevel='0'",
        "display='inline'",
    ] {
        let error = refused(
            &format!("<math {attribute}><mi>x</mi></math>"),
            TransformErrorKind::UnsupportedAttribute,
            "math",
            "/root/attributes/0",
        );
        assert_eq!(
            error.attribute.unwrap().local,
            attribute.split('=').next().unwrap()
        );
    }
}

#[test]
fn construct_attributes_refuse_even_when_empty_or_default() {
    for (name, markup, attribute) in [
        ("mi", "<mi mathvariant='italic'>x</mi>", "mathvariant"),
        ("mn", "<mn id=''>1</mn>", "id"),
        ("mtext", "<mtext class=''>text</mtext>", "class"),
        ("mo", "<mo stretchy='false'>+</mo>", "stretchy"),
        ("mrow", "<mrow data-label=''/>", "data-label"),
        (
            "mfrac",
            "<mfrac linethickness='1'><mi>x</mi><mi>y</mi></mfrac>",
            "linethickness",
        ),
        (
            "msub",
            "<msub subscriptshift='0'><mi>x</mi><mi>i</mi></msub>",
            "subscriptshift",
        ),
        (
            "msup",
            "<msup superscriptshift='0'><mi>x</mi><mi>i</mi></msup>",
            "superscriptshift",
        ),
    ] {
        let error = refused(
            &format!("<math>{markup}</math>"),
            TransformErrorKind::UnsupportedAttribute,
            name,
            "/root/children/0/attributes/0",
        );
        assert_eq!(error.attribute.unwrap().local, attribute);
    }
}

#[test]
fn namespaced_metadata_and_unknown_attributes_refuse() {
    for (source, namespace, name) in [
        (
            "<math><mi xml:lang='en'>x</mi></math>",
            "http://www.w3.org/XML/1998/namespace",
            "lang",
        ),
        (
            "<math xmlns:a='urn:extra'><mi a:flag=''>x</mi></math>",
            "urn:extra",
            "flag",
        ),
    ] {
        let error = refused(
            source,
            TransformErrorKind::UnsupportedAttribute,
            "mi",
            "/root/children/0/attributes/0",
        );
        let attribute = error.attribute.unwrap();
        assert_eq!(attribute.namespace.as_deref(), Some(namespace));
        assert_eq!(attribute.local, name);
    }
}

#[test]
fn unsupported_operator_text_is_never_coerced() {
    for text in ["−", "÷", "∑", " + ", "++", "", "\u{2062}"] {
        refused(
            &format!("<math><mo>{text}</mo></math>"),
            TransformErrorKind::UnsupportedOperator,
            "mo",
            "/root/children/0",
        );
    }
}

#[test]
fn binary_constructs_require_exactly_two_element_operands() {
    for name in ["mfrac", "msub", "msup"] {
        for content in ["", "<mi>x</mi>", "<mi>x</mi><mi>y</mi><mi>z</mi>"] {
            refused(
                &format!("<math><{name}>{content}</{name}></math>"),
                TransformErrorKind::InvalidStructure,
                name,
                "/root/children/0",
            );
        }
    }
}

#[test]
fn nested_token_markup_refuses_instead_of_flattening() {
    for name in ["mi", "mn", "mtext", "mo"] {
        refused(
            &format!("<math><{name}>a<mi>b</mi>c</{name}></math>"),
            TransformErrorKind::InvalidStructure,
            "mi",
            "/root/children/0/children/1",
        );
    }
}

#[test]
fn non_whitespace_and_unicode_spacing_outside_tokens_refuse() {
    for text in ["lost", "\u{a0}", "\u{2003}"] {
        refused(
            &format!("<math><mrow><mi>x</mi>{text}<mi>y</mi></mrow></math>"),
            TransformErrorKind::UnsupportedContent,
            "mrow",
            "/root/children/0/children/1",
        );
    }
}

#[test]
fn comments_refuse_inside_and_outside_the_document_element() {
    for (source, name, path) in [
        ("<!--note--><math/>", "math", "/before_root/0"),
        ("<math/><!--note-->", "math", "/after_root/0"),
        ("<math><!--note--></math>", "math", "/root/children/0"),
        (
            "<math><mi>a<!--note-->b</mi></math>",
            "mi",
            "/root/children/0/children/1",
        ),
    ] {
        refused(source, TransformErrorKind::UnsupportedContent, name, path);
    }
}

#[test]
fn nested_refusal_location_counts_indentation_and_preserves_namespace() {
    let error = refused(
        &format!("<math xmlns:m='{MATHML_NAMESPACE}'>\n<mfrac><mi>x</mi><mrow> <m:mstyle><mi>y</mi><mo>^</mo></m:mstyle></mrow></mfrac></math>"),
        TransformErrorKind::UnsupportedElement,
        "mstyle",
        "/root/children/1/children/1/children/1",
    );
    assert_eq!(error.element.namespace.as_deref(), Some(MATHML_NAMESPACE));
    assert_eq!(error.element.prefix.as_deref(), Some("m"));
}

#[test]
fn manually_constructed_non_math_root_refuses() {
    let mut document = parse("<math/>").unwrap();
    document.root.name.local = "mrow".into();
    let error = transform(&document).unwrap_err();
    assert_eq!(error.kind, TransformErrorKind::InvalidStructure);
    assert_eq!(error.element.local, "mrow");
    assert_eq!(error.location, "/root");
}

fn nested_document(depth: usize) -> MathmlDocument {
    let mut row = parse("<math><mrow/></math>").unwrap();
    let Node::Element(mut element) = row.root.children.pop().unwrap() else {
        panic!("expected element");
    };
    for _ in 2..depth {
        let mut parent = element.clone();
        parent.children = vec![Node::Element(element)];
        element = parent;
    }
    row.root.children = vec![Node::Element(element)];
    row
}

#[test]
fn manually_constructed_trees_enforce_depth_before_recursing() {
    assert!(transform(&nested_document(MAX_DEPTH)).is_ok());
    let error = transform(&nested_document(MAX_DEPTH + 1)).unwrap_err();
    assert_eq!(error.kind, TransformErrorKind::ResourceLimit);
    assert_eq!(error.element.local, "mrow");
    assert_eq!(error.location.matches("/children/0").count(), MAX_DEPTH);
    assert_eq!(error.code(), "RESOURCE_LIMIT");
}

#[test]
fn manually_constructed_trees_count_ignored_whitespace_toward_node_limit() {
    let mut document = parse("<math/>").unwrap();
    document.root.children = vec![Node::Text(" ".into()); MAX_NODES - 1];
    assert_eq!(transform(&document).unwrap(), Formula::Sequence(vec![]));
    document.root.children.push(Node::Text(" ".into()));
    let error = transform(&document).unwrap_err();
    assert_eq!(error.kind, TransformErrorKind::ResourceLimit);
    assert_eq!(error.location, format!("/root/children/{}", MAX_NODES - 1));
}
