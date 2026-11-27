use logifos_model::{Formula, MAX_DEPTH, MAX_NODES};
use logifos_syntax_mathml::{
    parse, transform, write, MathmlDocument, Node, TransformErrorKind, WriteErrorKind,
    MATHML_NAMESPACE,
};

const CONTAINERS: [&str; 4] = ["mstyle", "merror", "mpadded", "mphantom"];

fn symbol(text: &str) -> Formula {
    Formula::Symbol(text.into())
}

fn container(name: &str, children: Vec<Formula>) -> Formula {
    match name {
        "mstyle" => Formula::Style(children),
        "merror" => Formula::ErrorGroup(children),
        "mpadded" => Formula::Padded(children),
        "mphantom" => Formula::Phantom(children),
        _ => unreachable!(),
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
fn containers_preserve_empty_singleton_and_multiple_children() {
    for name in CONTAINERS {
        for count in 0..=3 {
            let children: Vec<_> = (0..count).map(|i| symbol(&i.to_string())).collect();
            let tokens: Vec<_> = (0..count).map(|i| format!("<mi>{i}</mi>")).collect();
            roundtrip(
                &format!("<{name}>\n {} \t</{name}>", tokens.join("\r\n")),
                container(name, children),
                &format!("<{name}>{}</{name}>", tokens.join("")),
            );
        }
    }
}

#[test]
fn nested_containers_roundtrip_in_tables_and_scripts() {
    let body = "<mtable><mtr><mtd><msubsup><mstyle><merror><mpadded><mphantom><mi>x</mi></mphantom></mpadded></merror></mstyle><mphantom></mphantom><mpadded><mi>y</mi></mpadded></msubsup></mtd></mtr></mtable>";
    let nested = Formula::Style(vec![Formula::ErrorGroup(vec![Formula::Padded(vec![
        Formula::Phantom(vec![symbol("x")]),
    ])])]);
    roundtrip(
        body,
        Formula::Table(vec![vec![Formula::Sequence(vec![Formula::SubSup(
            Box::new(nested),
            Box::new(Formula::Phantom(vec![])),
            Box::new(Formula::Padded(vec![symbol("y")])),
        )])]]),
        body,
    );
}

#[test]
fn presentation_names_accept_mathml_namespaces_and_refuse_foreign_ones() {
    for name in CONTAINERS.into_iter().chain(["semantics"]) {
        let expected = if name == "semantics" {
            symbol("x")
        } else {
            container(name, vec![symbol("x")])
        };
        for body in [
            format!("<{name} xmlns='{MATHML_NAMESPACE}'><mi>x</mi></{name}>"),
            format!("<m:{name} xmlns:m='{MATHML_NAMESPACE}'><m:mi>x</m:mi></m:{name}>"),
        ] {
            assert_eq!(
                transform(&parse(format!("<math>{body}</math>")).unwrap()).unwrap(),
                Formula::Sequence(vec![expected.clone()])
            );
        }
        refused(
            &format!("<f:{name} xmlns:f='urn:foreign'><mi>x</mi></f:{name}>"),
            TransformErrorKind::UnsupportedElement,
            "/root/children/0",
        );
    }
}

#[test]
fn unsupported_attributes_on_presentation_containers_still_refuse() {
    for name in CONTAINERS.into_iter().chain(["semantics"]) {
        for attribute in [
            "mathcolor='chartreuse'",
            "width='2em'",
            "data-note='keep'",
            "f:flag='yes' xmlns:f='urn:foreign'",
        ] {
            let error = transform(
                &parse(format!(
                    "<math><{name} {attribute}><mi>x</mi></{name}></math>"
                ))
                .unwrap(),
            )
            .unwrap_err();
            assert_eq!(error.kind, TransformErrorKind::UnsupportedAttribute);
            assert_eq!(error.element.local, name);
            assert_eq!(error.location, "/root/children/0/attributes/0");
            assert!(error.attribute.is_some());
        }
    }
}

#[test]
fn row_like_content_keeps_exact_refusal_paths() {
    for name in CONTAINERS {
        for (content, kind) in [
            ("<!--note-->", TransformErrorKind::UnsupportedContent),
            (
                "<annotation>meaning</annotation>",
                TransformErrorKind::UnsupportedElement,
            ),
        ] {
            refused(
                &format!("<{name}><mi>x</mi>{content}</{name}>"),
                kind,
                "/root/children/0/children/1",
            );
        }
        refused(
            &format!("<{name}><mi mathcolor='red'>x</mi></{name}>"),
            TransformErrorKind::UnsupportedAttribute,
            "/root/children/0/children/0/attributes/0",
        );
    }
}

#[test]
fn semantics_keeps_only_its_first_element_and_ignores_annotation_payloads() {
    roundtrip(
        "<semantics>\n<mphantom><mi>x</mi></mphantom>\n<annotation encoding='text/plain'>meaning<!--note--></annotation><annotation-xml encoding='application/xml'><foreign xmlns='urn:foreign' flag='yes'>content</foreign></annotation-xml><mo>^</mo></semantics>",
        Formula::Phantom(vec![symbol("x")]),
        "<mphantom><mi>x</mi></mphantom>",
    );
    roundtrip(
        "<semantics><mrow></mrow><mi>ignored</mi></semantics>",
        Formula::Sequence(vec![]),
        "<mrow></mrow>",
    );
    roundtrip(
        "<semantics><semantics><mi>x</mi></semantics></semantics>",
        symbol("x"),
        "<mi>x</mi>",
    );
}

#[test]
fn semantics_requires_a_presentational_first_element() {
    for body in ["<semantics/>", "<semantics> \n\t</semantics>"] {
        refused(
            body,
            TransformErrorKind::InvalidStructure,
            "/root/children/0",
        );
    }
    for name in ["annotation", "annotation-xml"] {
        refused(
            &format!("<semantics>\n<{name} encoding='text/plain'/><mi>x</mi></semantics>"),
            TransformErrorKind::InvalidStructure,
            "/root/children/0/children/1",
        );
        refused(
            &format!("<{name}/>"),
            TransformErrorKind::UnsupportedElement,
            "/root/children/0",
        );
    }
    // menclose is presentational now; its first-element acceptance is
    // covered by the menclose tests.
    refused(
        "<semantics><mi mathcolor='red'>x</mi><annotation/></semantics>",
        TransformErrorKind::UnsupportedAttribute,
        "/root/children/0/children/0/attributes/0",
    );
}

#[test]
fn semantics_keeps_row_like_whitespace_and_content_policy() {
    for content in ["lost", "&#xA0;", "<!--note-->"] {
        refused(
            &format!("<semantics>{content}<mi>x</mi></semantics>"),
            TransformErrorKind::UnsupportedContent,
            "/root/children/0/children/0",
        );
        refused(
            &format!("<semantics><mi>x</mi>{content}</semantics>"),
            TransformErrorKind::UnsupportedContent,
            "/root/children/0/children/1",
        );
    }
}

#[test]
fn container_writing_reports_invalid_descendants() {
    // The dictionary does not gate operator acceptance, so the refusal the
    // writer still reports for a descendant is an invalid XML character —
    // and diagnostics never echo the token contents.
    for name in CONTAINERS {
        let error = write(&Formula::Sequence(vec![container(
            name,
            vec![Formula::Operator("pri\u{7}vate".into())],
        )]))
        .unwrap_err();
        assert_eq!(error.kind, WriteErrorKind::InvalidCharacter);
        assert_eq!(error.location, "/children/0/children/0");
        assert!(!error.to_string().contains("pri"));
    }
}

#[test]
fn container_depth_limits_apply_to_transform_and_writer() {
    // The recursion at the exact depth limit needs frames the default
    // platform test stacks (windows-arm in particular) cannot spare.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(container_depth_limits_body)
        .expect("test thread spawns")
        .join()
        .expect("test thread succeeds");
}

fn container_depth_limits_body() {
    for name in CONTAINERS {
        let nested = (1..MAX_DEPTH).fold(container(name, vec![]), |child, _| {
            container(name, vec![child])
        });
        // Reserve the outer document sequence within the shared depth limit.
        assert_eq!(
            write(&Formula::Sequence(vec![nested])).unwrap_err().kind,
            WriteErrorKind::ResourceLimit
        );
        let at_limit = (2..MAX_DEPTH).fold(container(name, vec![]), |child, _| {
            container(name, vec![child])
        });
        let formula = Formula::Sequence(vec![at_limit]);
        let output = write(&formula).unwrap();
        assert_eq!(transform(&parse(output).unwrap()).unwrap(), formula);
        let mut document = parse("<math/>").unwrap();
        let mut child = document.root.clone();
        child.name.local = name.into();
        for _ in 1..MAX_DEPTH {
            let mut parent = child.clone();
            parent.children = vec![Node::Element(child)];
            child = parent;
        }
        document.root.children = vec![Node::Element(child)];
        assert_eq!(
            transform(&document).unwrap_err().kind,
            TransformErrorKind::ResourceLimit
        );
    }
}

#[test]
fn container_node_limits_apply_to_transform_and_writer() {
    for name in CONTAINERS {
        let leaf = parse("<math><mi/></math>").unwrap().root.children.remove(0);
        let mut group = parse(format!("<math><{name}/></math>")).unwrap().root;
        let Node::Element(mut container_element) = group.children.remove(0) else {
            unreachable!()
        };
        container_element.children = vec![leaf; MAX_NODES - 1];
        group.children = vec![Node::Element(container_element)];
        let document = MathmlDocument {
            before_root: vec![],
            root: group,
            after_root: vec![],
        };
        assert_eq!(
            transform(&document).unwrap_err().kind,
            TransformErrorKind::ResourceLimit
        );
        let formula = Formula::Sequence(vec![container(name, vec![symbol(""); MAX_NODES - 1])]);
        assert_eq!(
            write(&formula).unwrap_err().kind,
            WriteErrorKind::ResourceLimit
        );
    }
}
