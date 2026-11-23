use logifos_syntax_mathml::{
    parse, parse_with_limits, Element, Node, ParseErrorKind, ParseLimits, MATHML_NAMESPACE,
    XML_NAMESPACE,
};

fn element(node: &Node) -> &Element {
    match node {
        Node::Element(element) => element,
        other => panic!("expected element, found {other:?}"),
    }
}

fn refuses(source: impl AsRef<[u8]>, kind: ParseErrorKind) {
    let bytes = source.as_ref();
    let error = parse(bytes).unwrap_err();
    assert_eq!(error.kind, kind, "input {bytes:?}: {error}");
    assert!(error.offset <= bytes.len(), "{error}");
    assert!(error.to_string().len() < 200);
}

#[test]
fn namespace_prefixes_attributes_and_order() {
    let tree = parse(r#"<a:math xmlns:a="http://www.w3.org/1998/Math/MathML" xmlns:b="urn:labels" display="block" b:mark="yes"><a:mi xml:lang="el">β</a:mi><a:mo>+</a:mo><a:mn>5</a:mn></a:math>"#).unwrap();
    assert_eq!(tree.root.name.prefix.as_deref(), Some("a"));
    assert_eq!(tree.root.name.namespace.as_deref(), Some(MATHML_NAMESPACE));
    assert_eq!(tree.root.namespaces[0].prefix.as_deref(), Some("a"));
    assert_eq!(tree.root.namespaces[0].uri.as_ref(), MATHML_NAMESPACE);
    assert_eq!(tree.root.namespaces[1].uri.as_ref(), "urn:labels");
    assert_eq!(tree.root.attributes[0].name.local, "display");
    assert_eq!(tree.root.attributes[0].name.namespace, None);
    assert_eq!(tree.root.attributes[1].name.prefix.as_deref(), Some("b"));
    assert_eq!(
        tree.root.attributes[1].name.namespace.as_deref(),
        Some("urn:labels")
    );
    let children: Vec<_> = tree.root.children.iter().map(element).collect();
    assert_eq!(
        children
            .iter()
            .map(|e| e.name.local.as_str())
            .collect::<Vec<_>>(),
        ["mi", "mo", "mn"]
    );
    assert_eq!(children[0].children, [Node::Text("β".into())]);
    assert_eq!(
        children[0].attributes[0].name.namespace.as_deref(),
        Some(XML_NAMESPACE)
    );
}

#[test]
fn namespace_shadowing_undeclaration_and_restoration() {
    let tree = parse(r#"<math xmlns="http://www.w3.org/1998/Math/MathML" xmlns:s="urn:outer"><mrow xmlns="" xmlns:s="urn:inner"><s:item/><mi/></mrow><s:item/><mi/></math>"#).unwrap();
    let inner = element(&tree.root.children[0]);
    assert_eq!(inner.name.namespace, None);
    assert_eq!(inner.namespaces[0].uri.as_ref(), "");
    assert_eq!(
        element(&inner.children[0]).name.namespace.as_deref(),
        Some("urn:inner")
    );
    assert_eq!(element(&inner.children[1]).name.namespace, None);
    assert_eq!(
        element(&tree.root.children[1]).name.namespace.as_deref(),
        Some("urn:outer")
    );
    assert_eq!(
        element(&tree.root.children[2]).name.namespace.as_deref(),
        Some(MATHML_NAMESPACE)
    );
}

#[test]
fn declaration_after_attribute_and_alias_identity() {
    let tree = parse(r#"<m:math a:flag="one" xmlns:a="urn:one" xmlns:b="urn:one" b:other="two" xmlns:m="http://www.w3.org/1998/Math/MathML"/>"#).unwrap();
    assert_eq!(tree.root.attributes[0].name.prefix.as_deref(), Some("a"));
    assert_eq!(tree.root.attributes[1].name.prefix.as_deref(), Some("b"));
    assert_eq!(
        tree.root.attributes[0].name.namespace,
        tree.root.attributes[1].name.namespace
    );
}

#[test]
fn mixed_content_preserves_whitespace_comments_cdata_and_unicode() {
    let tree = parse("<!--before--><math><mtext> α <mi>x</mi> + <!--note--><![CDATA[<raw>&β]]> \u{301}</mtext></math><!--after-->").unwrap();
    assert_eq!(tree.before_root, ["before"]);
    assert_eq!(tree.after_root, ["after"]);
    let text = element(&tree.root.children[0]);
    assert_eq!(text.children.len(), 6);
    assert_eq!(text.children[0], Node::Text(" α ".into()));
    assert_eq!(element(&text.children[1]).name.local, "mi");
    assert_eq!(text.children[2], Node::Text(" + ".into()));
    assert_eq!(text.children[3], Node::Comment("note".into()));
    assert_eq!(text.children[4], Node::Text("<raw>&β".into()));
    assert_eq!(text.children[5], Node::Text(" \u{301}".into()));
}

#[test]
fn references_and_xml_normalization() {
    let tree = parse("<math note='a\t b\r\nc&#x9;&#13;&quot;&apos;&amp;&lt;&gt;'><mtext>a\r\nb\rc&#xD;&#x1D6FC;&#945;</mtext><![CDATA[\r\nz\r]]></math>").unwrap();
    assert_eq!(tree.root.attributes[0].value, "a  b c\t\r\"'&<>");
    assert_eq!(
        element(&tree.root.children[0]).children,
        [Node::Text("a\nb\nc\r𝛼α".into())]
    );
    assert_eq!(tree.root.children[1], Node::Text("\nz\n".into()));
    let tree = parse("<math>&amp;lt;</math>").unwrap();
    assert_eq!(tree.root.children, [Node::Text("&lt;".into())]);
}

#[test]
fn preserves_unknown_syntax_and_inherited_presentation() {
    let tree = parse(r##"<math mathcolor="#aabbcc" dir="rtl"><mstyle scriptlevel="+2" mathvariant="bold"><mi data-note="keep">k</mi><ext:future xmlns:ext="urn:extension" ext:flag="yes"/></mstyle></math>"##).unwrap();
    assert_eq!(tree.root.attributes[0].value, "#aabbcc");
    let style = element(&tree.root.children[0]);
    assert_eq!(style.attributes[0].value, "+2");
    assert_eq!(style.attributes[1].value, "bold");
    let token = element(&style.children[0]);
    assert_eq!(token.attributes.len(), 1);
    assert_eq!(token.attributes[0].name.local, "data-note");
    assert_eq!(
        element(&style.children[1]).name.namespace.as_deref(),
        Some("urn:extension")
    );
}

#[test]
fn accepts_utf8_declaration_bom_and_empty_legacy_math() {
    let tree =
        parse("\u{feff}<?xml version='1.0' encoding='uTf-8' standalone='yes'?><math/>").unwrap();
    assert_eq!(tree.root.name.local, "math");
    assert_eq!(tree.root.name.namespace, None);
    assert!(tree.root.children.is_empty());
    assert_eq!(parse("<math/>").unwrap(), parse("<math></math>").unwrap());
}

#[test]
fn malformed_xml_is_refused() {
    for source in [
        "",
        " ",
        "<!--only-->",
        "<math>",
        "<math><mi/></math",
        "<math></mi>",
        "<math><mi></math></mi>",
        "<math/><math/>",
        "before<math/>",
        "<math/>after",
        "<math a=bare/>",
        "<math a='unclosed/>",
        "<math a='<'/>",
        "<math a='1' a='2'/>",
        "<math><!--bad--comment--></math>",
        "<math>]]></math>",
        "<math>&#;</math>",
        "<math>&#x;</math>",
        "<math>&#x110000;</math>",
        "<math>&#xD800;</math>",
        "<math>&#0;</math>",
        "<math>&#xFFFE;</math>",
        "<math>&#-1;</math>",
        "<math>&#+65;</math>",
        "<math>&#x+41;</math>",
        "<math>&amp</math>",
        "<math>\u{0}</math>",
        "<math>\u{b}</math>",
        "<math>\u{fffe}</math>",
        "<math a='&#0;'/>",
        "<math><![CDATA[unfinished</math>",
        "<?xml version='1.0'?><math/><?xml version='1.0'?>",
    ] {
        refuses(source, ParseErrorKind::MalformedXml);
    }
}

#[test]
fn namespace_errors_are_refused() {
    for source in [
        "<m:math/>",
        "<math p:attr='x'/>",
        "<math xmlns:p='urn:p'><q:mi/></math>",
        "<math xmlns:p=''/>",
        "<math xmlns:xmlns='urn:bad'/>",
        "<math xmlns='http://www.w3.org/2000/xmlns/'/>",
        "<math xmlns:p='http://www.w3.org/2000/xmlns/'/>",
        "<math xmlns='http://www.w3.org/XML/1998/namespace'/>",
        "<math xmlns:p='http://www.w3.org/XML/1998/namespace'/>",
        "<math xmlns:xml='urn:bad'/>",
        "<xmlns:math/>",
        "<math xmlns:a='urn:s' xmlns:b='urn:s' a:n='1' b:n='2'/>",
        "<math xmlns:a='urn:s' xmlns:a='urn:t'/>",
        "<math xmlns:a='urn:s' xmlns:b='urn:s'><a:mi></b:mi></math>",
        "<math><a:b:c/></math>",
        "<math a:b:c='x'/>",
    ] {
        refuses(source, ParseErrorKind::MalformedXml);
    }
}

#[test]
fn unsafe_facilities_are_refused() {
    for source in [
        "<!DOCTYPE math><math/>",
        "<!DOCTYPE math SYSTEM 'file:///missing.dtd'><math/>",
        "<!DOCTYPE math PUBLIC 'id' 'https://invalid.example/dtd'><math/>",
        "<!DOCTYPE math [<!ENTITY a 'expanded'>]><math>&a;</math>",
        "<!DOCTYPE math [<!ENTITY a SYSTEM 'file:///missing'>]><math>&a;</math>",
        "<!DOCTYPE math [<!ENTITY % a SYSTEM 'https://invalid.example/e'>%a;]><math/>",
        "<math>&alpha;</math>", "<math a='&custom;'/>",
        "<?xml-stylesheet href='https://invalid.example/style'?><math/>",
        "<math><?work ignore?></math>",
        "<math xmlns:i='http://www.w3.org/2001/XInclude'><i:include href='file:///missing'/></math>",
        "<math><mglyph src='https://invalid.example/image'/></math>",
        "<math href='#local'/>", "<math altimg='file:///image'/>",
        "<math xml:base='https://invalid.example/'/>",
        "<math xmlns:x='http://www.w3.org/1999/xlink' x:href='file:///missing'/>",
    ] {
        refuses(source, ParseErrorKind::UnsafeXml);
    }
    // Markup-looking content is inert text or a comment, not a facility.
    assert!(
        parse("<math><![CDATA[<!DOCTYPE math>&custom;]]><!--<!DOCTYPE math>--></math>").is_ok()
    );
}

#[test]
fn unsupported_roots_and_encodings_are_typed() {
    for source in [
        "<mi>x</mi>",
        "<math xmlns='urn:other'/>",
        "<?xml version='1.1'?><math/>",
        "<?xml version='1.0' encoding='UTF-16'?><math/>",
    ] {
        refuses(source, ParseErrorKind::UnsupportedSyntax);
    }
    refuses(
        [b'<', b'm', b'a', b't', b'h', b'>', 0xff],
        ParseErrorKind::InvalidUtf8,
    );
}

#[test]
fn byte_limits_measure_utf8_and_accept_exact_boundary() {
    let source = "<math>π</math>";
    let limits = ParseLimits {
        source_bytes: source.len(),
        ..ParseLimits::default()
    };
    assert!(parse_with_limits(source, limits).is_ok());
    let error = parse_with_limits(
        source,
        ParseLimits {
            source_bytes: source.len() - 1,
            ..limits
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, ParseErrorKind::ResourceLimit);
    let hard = ParseLimits::default().source_bytes;
    let exact = format!("<math>{}</math>", " ".repeat(hard - 13));
    assert_eq!(exact.len(), hard);
    assert!(parse(&exact).is_ok());
    refuses(exact + " ", ParseErrorKind::ResourceLimit);
}

#[test]
fn depth_limits_include_empty_elements_text_and_comments() {
    let limits = ParseLimits {
        tree_depth: 2,
        ..ParseLimits::default()
    };
    for source in [
        "<math><mi/></math>",
        "<math>x</math>",
        "<math><mrow/></math>",
    ] {
        assert!(parse_with_limits(source, limits).is_ok());
    }
    for source in [
        "<math><mrow><mi/></mrow></math>",
        "<math><mi>x</mi></math>",
        "<math><mi><!--c--></mi></math>",
    ] {
        assert_eq!(
            parse_with_limits(source, limits).unwrap_err().kind,
            ParseErrorKind::ResourceLimit
        );
    }
    let exact = format!(
        "<math>{}{}</math>",
        "<mrow>".repeat(127),
        "</mrow>".repeat(127)
    );
    assert!(parse(exact).is_ok());
    let excessive = format!(
        "<math>{}{}</math>",
        "<mrow>".repeat(20_000),
        "</mrow>".repeat(20_000)
    );
    refuses(excessive, ParseErrorKind::ResourceLimit);
}

#[test]
fn node_limits_count_text_cdata_comments_and_document_comments() {
    let limits = ParseLimits {
        tree_nodes: 3,
        ..ParseLimits::default()
    };
    for source in [
        "<math><mi/><mn/></math>",
        "<math>a<![CDATA[b]]></math>",
        "<!--a--><math/><!--b-->",
    ] {
        assert!(parse_with_limits(source, limits).is_ok());
    }
    for source in [
        "<math><mi/><mn/><mo/></math>",
        "<math>a<![CDATA[b]]><!--c--></math>",
        "<!--a--><math/><!--b--><!--c-->",
    ] {
        assert_eq!(
            parse_with_limits(source, limits).unwrap_err().kind,
            ParseErrorKind::ResourceLimit
        );
    }
    let exact = format!("<math>{}</math>", "<mi/>".repeat(99_999));
    assert!(parse(&exact).is_ok());
    refuses(
        exact.replace("</math>", "<mi/></math>"),
        ParseErrorKind::ResourceLimit,
    );
}

#[test]
fn callers_cannot_raise_or_disable_limits() {
    let hard = ParseLimits::default();
    for limits in [
        ParseLimits {
            source_bytes: 0,
            ..hard
        },
        ParseLimits {
            source_bytes: hard.source_bytes + 1,
            ..hard
        },
        ParseLimits {
            tree_depth: 0,
            ..hard
        },
        ParseLimits {
            tree_depth: hard.tree_depth + 1,
            ..hard
        },
        ParseLimits {
            tree_nodes: 0,
            ..hard
        },
        ParseLimits {
            tree_nodes: hard.tree_nodes + 1,
            ..hard
        },
    ] {
        let error = parse_with_limits("<math/>", limits).unwrap_err();
        assert_eq!(error.kind, ParseErrorKind::InvalidLimits);
        assert_eq!(error.code(), "INVALID_REQUEST");
    }
    let limits = ParseLimits {
        tree_depth: 1,
        tree_nodes: 1,
        ..hard
    };
    assert!(parse_with_limits("<math/>", limits).is_ok());
}

#[test]
fn diagnostics_are_bounded_and_positions_are_utf8_bytes() {
    let source = "<math>α&private;</math>";
    let error = parse(source).unwrap_err();
    assert_eq!(error.offset, source.find('&').unwrap());
    assert!(!error.to_string().contains("private"));
    assert_eq!(error.code(), "UNSUPPORTED_SYNTAX");
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(parse([0xff]).unwrap_err().code(), "INVALID_UTF8");
    assert_eq!(
        parse(vec![0xff; 1_048_577]).unwrap_err().code(),
        "RESOURCE_LIMIT"
    );
    assert!(
        parse("<math/>").is_ok(),
        "a refusal must not affect later calls"
    );
}

/// Shared case entries use the conformance manifest's case shape. Selection by
/// `LOGIFOS_MATHML_CASE` is exact and must never turn a missing case into a pass.
#[test]
fn shared_cases() {
    let cases = logifos_json::decode(include_bytes!("cases.json")).unwrap();
    let cases = cases.as_array().unwrap();
    assert!(!cases.is_empty());
    let selected = std::env::var("LOGIFOS_MATHML_CASE").ok();
    let mut ran = 0;
    for case in cases {
        let id = case["id"].as_str().unwrap();
        if selected.as_deref().is_some_and(|selected| selected != id) {
            continue;
        }
        let path = case["path"].as_str().unwrap();
        let source = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join(path),
        )
        .unwrap();
        let assertion = &case["expected"]["assertion"];
        let hard = ParseLimits::default();
        let limits = ParseLimits {
            source_bytes: assertion["source_bytes"]
                .as_u64()
                .map_or(hard.source_bytes, |n| n as usize),
            tree_depth: assertion["tree_depth"]
                .as_u64()
                .map_or(hard.tree_depth, |n| n as usize),
            ..hard
        };
        match parse_with_limits(source, limits) {
            Ok(tree) => {
                assert_eq!(case["expected"]["kind"], "syntax", "{id}");
                assert_eq!(tree.root.name.local, "math", "{id}");
                if let Some(namespace) = assertion["root_namespace"].as_str() {
                    assert_eq!(tree.root.name.namespace.as_deref(), Some(namespace), "{id}");
                }
                if let Some(prefix) = assertion["root_prefix"].as_str() {
                    assert_eq!(tree.root.name.prefix.as_deref(), Some(prefix), "{id}");
                }
                if let Some(local) = assertion["first_child"].as_str() {
                    assert_eq!(element(&tree.root.children[0]).name.local, local, "{id}");
                }
                if let Some(local) = assertion["first_attribute"].as_str() {
                    assert_eq!(tree.root.attributes[0].name.local, local, "{id}");
                }
            }
            Err(error) => {
                assert_eq!(case["expected"]["kind"], "refusal", "{id}");
                assert_eq!(
                    format!("{:?}", error.kind),
                    assertion["error_kind"].as_str().unwrap(),
                    "{id}"
                );
            }
        }
        ran += 1;
    }
    assert!(ran > 0, "no matching MathML case: {selected:?}");
}

#[test]
fn namespace_storage_does_not_amplify_long_uri_references() {
    let uri = format!("urn:{}", "u".repeat(32_000));
    let source = format!(
        "<math xmlns:p='{uri}'>{}</math>",
        "<p:item p:flag='v'/>".repeat(2_000)
    );
    let tree = parse(source).unwrap();
    let declared = &tree.root.namespaces[0].uri;
    for child in &tree.root.children {
        let child = element(child);
        assert!(std::sync::Arc::ptr_eq(
            declared,
            child.name.namespace.as_ref().unwrap()
        ));
        assert!(std::sync::Arc::ptr_eq(
            declared,
            child.attributes[0].name.namespace.as_ref().unwrap()
        ));
    }
    let mut source = String::from("<math");
    for index in 0..2_000 {
        source.push_str(&format!(" xmlns:p{index}='urn:{index}'"));
    }
    for index in (0..2_000).rev() {
        source.push_str(&format!(" p{index}:flag='v'"));
    }
    source.push_str("/>");
    assert_eq!(parse(source).unwrap().root.attributes.len(), 2_000);
}
