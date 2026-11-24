use std::collections::{BTreeMap, BTreeSet};

use logifos_layout::layout::PlacedGlyph;
use logifos_layout::{Bounds, BoxTree, GeometryError, Kern, Metrics, Point, StackItem};
use logifos_svg::{emit, SvgDocument, SvgError, SvgOptions};
use xmlparser::{ElementEnd, Token, Tokenizer};

#[derive(Debug)]
struct Element {
    name: String,
    attributes: BTreeMap<String, String>,
    parent: Option<usize>,
}

struct Parsed {
    elements: Vec<Element>,
    title: Option<String>,
}

// Decode references once, normalize literal line endings, and normalize literal
// attribute whitespace without changing whitespace from character references.
fn decode_xml(mut input: &str, attribute: bool) -> String {
    let mut output = String::new();
    while !input.is_empty() {
        let value = if let Some(reference) = input.strip_prefix('&') {
            let (reference, rest) = reference
                .split_once(';')
                .expect("unterminated XML reference");
            input = rest;
            match reference {
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "quot" => '"',
                "apos" => '\'',
                _ => {
                    let numeric = reference.strip_prefix('#').expect("unknown XML entity");
                    let (digits, radix) =
                        numeric.strip_prefix('x').map_or((numeric, 10), |s| (s, 16));
                    assert!(
                        !digits.is_empty() && digits.chars().all(|digit| digit.is_digit(radix))
                    );
                    u32::from_str_radix(digits, radix)
                        .ok()
                        .and_then(char::from_u32)
                        .expect("invalid XML character reference")
                }
            }
        } else {
            let mut value = input.chars().next().unwrap();
            input = &input[value.len_utf8()..];
            if value == '\r' {
                input = input.strip_prefix('\n').unwrap_or(input);
                value = '\n';
            }
            if attribute && matches!(value, '\t' | '\n') {
                value = ' ';
            }
            value
        };
        assert!(
            matches!(value, '\t' | '\n' | '\r')
                || (value >= ' ' && !matches!(value, '\u{fffe}' | '\u{ffff}')),
            "invalid XML character"
        );
        output.push(value);
    }
    output
}

fn parse(document: &SvgDocument) -> Parsed {
    parse_xml(document.as_str())
}

// The tokenizer checks syntax; independently enforce the emitted vocabulary,
// balanced nesting, one root, distinct attributes, and unique IDs.
fn parse_xml(document: &str) -> Parsed {
    let mut elements: Vec<Element> = Vec::new();
    let mut parents: Vec<usize> = Vec::new();
    let mut pending = None;
    let mut title = None;
    let mut ids = BTreeSet::new();
    let mut consumed = 0;
    for token in Tokenizer::from(document) {
        let token = token.expect("invalid XML syntax");
        let gap = &document[consumed..token.span().start()];
        assert!(
            gap.is_empty()
                || (pending.is_some()
                    && gap
                        .bytes()
                        .all(|value| matches!(value, b' ' | b'\t' | b'\r' | b'\n'))),
            "unparsed XML content"
        );
        consumed = token.span().end();
        match token {
            Token::ElementStart { prefix, local, .. } => {
                assert!(prefix.is_empty(), "unexpected element prefix");
                assert!(pending.is_none());
                if let Some(&parent) = parents.last() {
                    assert_ne!(elements[parent].name, "title", "markup inside title");
                } else {
                    assert!(elements.is_empty(), "multiple XML roots");
                }
                let index = elements.len();
                elements.push(Element {
                    name: local.as_str().into(),
                    attributes: BTreeMap::new(),
                    parent: parents.last().copied(),
                });
                pending = Some(index);
            }
            Token::Attribute {
                prefix,
                local,
                value,
                ..
            } => {
                assert!(prefix.is_empty(), "unexpected attribute prefix");
                let element = &mut elements[pending.expect("attribute outside start tag")];
                let name = &element.name;
                let key = local.as_str();
                let allowed = match name.as_str() {
                    "svg" => &["xmlns", "width", "height", "viewBox"][..],
                    "g" => &["id", "transform", "fill"][..],
                    "rect" => &["x", "y", "width", "height"][..],
                    "path" => &["d"][..],
                    "title" => &[][..],
                    other => panic!("unexpected SVG element: {other}"),
                };
                assert!(allowed.contains(&key), "unexpected {name}.{key}");
                let value = decode_xml(value.as_str(), true);
                if key == "id" {
                    assert!(ids.insert(value.clone()), "duplicate ID");
                }
                assert!(
                    element.attributes.insert(key.into(), value).is_none(),
                    "duplicate attribute"
                );
            }
            Token::ElementEnd {
                end: ElementEnd::Open | ElementEnd::Empty,
                ..
            } => {
                let index = pending.take().expect("missing start tag");
                let name = elements[index].name.as_str();
                assert!(matches!(name, "svg" | "g" | "rect" | "path" | "title"));
                if name == "title" {
                    assert!(title.replace(String::new()).is_none(), "duplicate title");
                }
                if matches!(
                    token,
                    Token::ElementEnd {
                        end: ElementEnd::Open,
                        ..
                    }
                ) {
                    parents.push(index);
                }
            }
            Token::ElementEnd {
                end: ElementEnd::Close(prefix, local),
                ..
            } => {
                assert!(prefix.is_empty(), "unexpected closing prefix");
                assert!(pending.is_none());
                let index = parents.pop().expect("unbalanced XML");
                assert_eq!(elements[index].name, local.as_str(), "mismatched XML tags");
            }
            Token::Text { text } => {
                let index = *parents.last().expect("text outside root");
                assert_eq!(elements[index].name, "title", "unexpected text");
                title
                    .as_mut()
                    .unwrap()
                    .push_str(&decode_xml(text.as_str(), false));
            }
            token => panic!("unexpected XML token: {token:?}"),
        }
    }
    assert_eq!(consumed, document.len(), "unparsed trailing XML content");
    assert!(pending.is_none(), "unfinished start tag");
    assert!(parents.is_empty());
    assert_eq!(elements.iter().filter(|e| e.parent.is_none()).count(), 1);
    assert_eq!(elements[0].name, "svg");
    assert_eq!(
        elements[0].attributes["xmlns"],
        "http://www.w3.org/2000/svg"
    );
    Parsed { elements, title }
}

#[test]
fn xml_checks_reject_malformed_documents() {
    let root = "<svg xmlns=\"http://www.w3.org/2000/svg\"/>";
    for document in [
        String::new(),
        "<svg".into(),
        "<svg xmlns=\"http://www.w3.org/2000/svg\">".into(),
        format!("{root}trailing"),
        format!("{root} "),
        format!("{root}<g/>"),
        format!("{root}<!--trailing-->"),
        format!("{root}<"),
        format!(" {root}"),
    ] {
        assert!(
            std::panic::catch_unwind(|| parse_xml(&document)).is_err(),
            "accepted malformed XML: {document:?}"
        );
    }
    for body in [
        "<g></svg></g>",
        "</g>",
        "<g>",
        "<g fill=#000000/>",
        "<g fill=\"unterminated/>",
        "<g fill=\"a\" fill=\"b\"/>",
        "<g fill=\"<\"/>",
        "<g onload=\"x\"/>",
        "<path d=\"M0 0 Z\" onload=\"x\"/>",
        "<g x:fill=\"a\"/>",
        "<x:g/>",
        "<g></x:g>",
        "<g id=\"same\"/><g id=\"same\"/>",
        "<g fill=\"&unknown;\"/>",
        "<g fill=\"&amp\"/>",
        "<title>&unknown;</title>",
        "<title>&amp</title>",
        "<title>&#0;</title>",
        "<title>&#xD800;</title>",
        "<title>&#x110000;</title>",
        "<title>&#xFFFE;</title>",
        "<title>]]></title>",
        "<title><g/></title>",
        "<title/><title/>",
        "<script/>",
    ] {
        let document = format!("<svg xmlns=\"http://www.w3.org/2000/svg\">{body}</svg>");
        assert!(
            std::panic::catch_unwind(|| parse_xml(&document)).is_err(),
            "accepted malformed SVG: {document:?}"
        );
    }
}

#[test]
fn xml_checks_normalize_attributes_and_decode_title_once() {
    let parsed = parse_xml(concat!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" ",
        "width=\"a\r\nb\rc\td\ne&#9;&#10;&#13;&#x20;&amp;&lt;&gt;&quot;&apos;\">",
        "<title>α\r\nβ\rγ &amp;amp; &lt;&gt;&quot;&apos; &#13;&#10;&#9;&#x1F980;</title>",
        "</svg>",
    ));
    assert_eq!(
        parsed.elements[0].attributes["width"],
        "a b c d e\t\n\r &<>\"'"
    );
    assert_eq!(
        parsed.title.as_deref(),
        Some("α\nβ\nγ &amp; <>\"' \r\n\t🦀")
    );
    let empty_title = parse_xml("<svg xmlns=\"http://www.w3.org/2000/svg\"><title/></svg>");
    assert_eq!(empty_title.title.as_deref(), Some(""));
}

fn numbers(value: &str) -> Vec<f64> {
    value
        .split_whitespace()
        .map(|value| value.parse().unwrap())
        .collect()
}

impl Parsed {
    // Independently compose SVG group translations to recover painted geometry.
    fn rectangles(&self) -> Vec<[f64; 4]> {
        self.elements
            .iter()
            .filter(|element| element.name == "rect")
            .map(|element| {
                let mut x = element.attributes["x"].parse::<f64>().unwrap();
                let mut y = element.attributes["y"].parse::<f64>().unwrap();
                let mut parent = element.parent;
                while let Some(index) = parent {
                    let ancestor = &self.elements[index];
                    if let Some(transform) = ancestor.attributes.get("transform") {
                        let values = numbers(
                            transform
                                .strip_prefix("translate(")
                                .unwrap()
                                .strip_suffix(')')
                                .unwrap(),
                        );
                        assert_eq!(values.len(), 2);
                        x += values[0];
                        y += values[1];
                    }
                    parent = ancestor.parent;
                }
                [
                    x,
                    y,
                    element.attributes["width"].parse().unwrap(),
                    element.attributes["height"].parse().unwrap(),
                ]
            })
            .collect()
    }

    fn assert_viewport(&self, expected: [f64; 4]) {
        let root = &self.elements[0].attributes;
        assert_eq!(numbers(&root["viewBox"]), expected);
        assert_eq!(root["width"], format!("{}px", expected[2]));
        assert_eq!(root["height"], format!("{}px", expected[3]));
        // Every painted edge lies inside the viewport, independently of metadata.
        for [x, y, width, height] in self.rectangles() {
            assert!(x >= expected[0] && y >= expected[1]);
            assert!(x + width <= expected[0] + expected[2]);
            assert!(y + height <= expected[1] + expected[3]);
        }
    }
}

fn rule(width: f64, height: f64, depth: f64) -> BoxTree {
    BoxTree::rule(width, height, depth).unwrap()
}

fn kern(amount: f64) -> StackItem {
    Kern::new(amount).unwrap().into()
}

fn emit_default(tree: &BoxTree) -> SvgDocument {
    emit(tree, &[], &SvgOptions::default()).unwrap()
}

#[test]
fn thin_rule_has_exact_bytes_and_unchanged_metrics() {
    let tree = rule(9.5, 0.375, 0.125);
    let document = emit_default(&tree);
    assert_eq!(
        document.as_str(),
        concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"9.5px\" height=\"0.5px\" viewBox=\"0 0 9.5 0.5\">",
            "<g fill=\"#000000\"><g id=\"box-0\" transform=\"translate(0 0)\">",
            "<rect x=\"0\" y=\"0\" width=\"9.5\" height=\"0.5\"/></g></g></svg>",
        )
    );
    assert_eq!(document.metrics(), Metrics::new(9.5, 0.375, 0.125).unwrap());
    assert_eq!(document.ink_bounds(), tree.ink_bounds());
    assert_eq!(
        document.viewport(),
        Bounds::new(0.0, 0.0, 9.5, 0.5).unwrap()
    );
    parse(&document).assert_viewport([0.0, 0.0, 9.5, 0.5]);
}

#[test]
fn nested_rows_and_columns_compose_positions_and_baselines() {
    // Row baseline=5, depth=1: its short rule moves down to y=3.
    // Column positions the row at y=1.25 and the bottom rule at y=9.25.
    let row = BoxTree::hbox(vec![
        rule(3.0, 2.0, 1.0).into(),
        kern(0.75),
        rule(2.0, 5.0, 0.0).into(),
    ])
    .unwrap();
    let tree = BoxTree::vbox(
        vec![
            kern(1.25),
            row.into(),
            kern(2.0),
            rule(7.0, 0.5, 0.0).into(),
        ],
        6.25,
    )
    .unwrap();
    let document = emit_default(&tree);
    let parsed = parse(&document);
    assert_eq!(
        parsed.rectangles(),
        vec![
            [0.0, 4.25, 3.0, 3.0],
            [3.75, 1.25, 2.0, 5.0],
            [0.0, 9.25, 7.0, 0.5]
        ]
    );
    parsed.assert_viewport([0.0, 0.0, 7.0, 9.75]);
    assert_eq!(document.metrics(), Metrics::new(7.0, 6.25, 3.5).unwrap());
    assert_eq!(document.metrics().baseline(), 6.25);
    let ids: Vec<_> = parsed
        .elements
        .iter()
        .filter_map(|e| e.attributes.get("id").map(String::as_str))
        .collect();
    assert_eq!(ids, ["box-0", "box-1", "box-2", "box-3", "box-4"]);
}

#[test]
fn negative_kerns_expand_viewport_on_all_four_sides() {
    // Both advances end at zero while the rule occupies (-2,-3)..(3,4).
    let row = BoxTree::hbox(vec![kern(-2.0), rule(5.0, 6.0, 1.0).into(), kern(-3.0)]).unwrap();
    let tree = BoxTree::vbox(vec![kern(-3.0), row.into(), kern(-4.0)], 0.0).unwrap();
    let document = emit_default(&tree);
    assert_eq!(document.metrics(), Metrics::default());
    assert_eq!(
        document.ink_bounds(),
        Some(Bounds::new(-2.0, -3.0, 5.0, 7.0).unwrap())
    );
    let parsed = parse(&document);
    assert_eq!(parsed.rectangles(), [[-2.0, -3.0, 5.0, 7.0]]);
    parsed.assert_viewport([-2.0, -3.0, 5.0, 7.0]);
}

#[test]
fn positive_spacing_keeps_logical_area_beyond_ink() {
    let tree = BoxTree::hbox(vec![kern(2.0), rule(3.0, 1.0, 0.0).into(), kern(4.0)]).unwrap();
    let document = emit_default(&tree);
    assert_eq!(
        document.ink_bounds(),
        Some(Bounds::new(2.0, 0.0, 3.0, 1.0).unwrap())
    );
    parse(&document).assert_viewport([0.0, 0.0, 9.0, 1.0]);
}

#[test]
fn overlapping_rules_preserve_input_paint_order() {
    let tree = BoxTree::hbox(vec![
        rule(5.0, 3.0, 0.0).into(),
        kern(-4.0),
        rule(2.0, 1.0, 0.0).into(),
    ])
    .unwrap();
    let parsed = parse(&emit_default(&tree));
    assert_eq!(
        parsed.rectangles(),
        [[0.0, 0.0, 5.0, 3.0], [1.0, 2.0, 2.0, 1.0]]
    );
    parsed.assert_viewport([0.0, 0.0, 5.0, 3.0]);
}

#[test]
fn empty_stacks_and_zero_area_rules_emit_no_rectangles() {
    for tree in [
        BoxTree::hbox(vec![]).unwrap(),
        BoxTree::vbox(vec![], 0.0).unwrap(),
        rule(0.0, 3.0, 1.0),
        rule(2.0, 0.0, 0.0),
        BoxTree::hbox(vec![kern(4.0)]).unwrap(),
        BoxTree::vbox(vec![kern(5.0)], 2.0).unwrap(),
        BoxTree::space(Metrics::new(3.0, 4.0, 2.0).unwrap()),
    ] {
        let document = emit_default(&tree);
        assert_eq!(document.ink_bounds(), None);
        let parsed = parse(&document);
        assert!(parsed.rectangles().is_empty());
        parsed.assert_viewport([
            0.0,
            0.0,
            tree.metrics().width(),
            tree.metrics().total_height(),
        ]);
    }
}

#[test]
fn inkless_leaf_reserves_advance_and_baseline_without_painting() {
    let tree = BoxTree::hbox(vec![
        BoxTree::space(Metrics::new(4.0, 8.0, 2.0).unwrap()).into(),
        rule(2.0, 1.0, 0.0).into(),
    ])
    .unwrap();
    let parsed = parse(&emit_default(&tree));
    assert_eq!(parsed.rectangles(), [[4.0, 7.0, 2.0, 1.0]]);
    parsed.assert_viewport([0.0, 0.0, 6.0, 10.0]);
}

#[test]
fn ink_bearing_leaves_paint_their_placed_glyph_paths_with_exact_bytes() {
    let bounds = Bounds::new(-1.0, -2.0, 5.0, 7.0).unwrap();
    let tree = BoxTree::leaf(Metrics::new(3.0, 4.0, 1.0).unwrap(), Some(bounds));
    let glyph = placed_glyph(
        Point::new(2.0, 4.0).unwrap(),
        Some(font_outline_px(vec![vec![
            point_px(1.0, 0.0, true),
            point_px(3.0, -5.0, false),
            point_px(5.0, 0.0, true),
        ]])),
    );
    let document = emit(&tree, &[glyph], &SvgOptions::default()).unwrap();
    // Font orientation (y up) flips around the baseline origin (2, 4):
    // (1,0)->(3,4), off-curve (3,-5)->(5,9), (5,0)->(7,4).
    assert_eq!(
        document.as_str(),
        concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"5px\" height=\"7px\" viewBox=\"-1 -2 5 7\">",
            "<g fill=\"#000000\"><g id=\"box-0\" transform=\"translate(0 0)\"></g>",
            "<g id=\"glyphs\"><path d=\"M3 4 Q5 9 7 4 Z\"/></g></g></svg>",
        )
    );
}

#[test]
fn off_curve_runs_get_one_implied_midpoint_per_pair() {
    let tree = BoxTree::space(Metrics::new(3.0, 4.0, 1.0).unwrap());
    let glyph = placed_glyph(
        Point::new(0.0, 4.0).unwrap(),
        Some(font_outline_px(vec![vec![
            point_px(0.0, 0.0, false),
            point_px(2.0, 2.0, false),
            point_px(4.0, 0.0, false),
        ]])),
    );
    let document = emit(&tree, &[glyph], &SvgOptions::default()).unwrap();
    // All off-curve: implied midpoints at (1,3), (3,3) and the wrap midpoint
    // (2,4); the walk starts at the first midpoint and closes back onto it.
    assert!(document
        .as_str()
        .contains("<path d=\"M1 3 Q2 2 3 3 Q4 4 2 4 Q0 4 1 3 Z\"/>"));
}

#[test]
fn a_quadratic_landing_on_the_start_point_closes_the_contour() {
    // Alternating on/off with no implied midpoints: the final Q lands back on
    // the start point and must end the walk instead of wrapping past it.
    let tree = BoxTree::space(Metrics::new(4.0, 4.0, 2.0).unwrap());
    let glyph = placed_glyph(
        Point::new(0.0, 2.0).unwrap(),
        Some(font_outline_px(vec![vec![
            point_px(0.0, 0.0, true),
            point_px(2.0, 2.0, false),
            point_px(4.0, 0.0, true),
            point_px(2.0, -2.0, false),
        ]])),
    );
    let document = emit(&tree, &[glyph], &SvgOptions::default()).unwrap();
    assert!(document
        .as_str()
        .contains("<path d=\"M0 2 Q2 0 4 2 Q2 4 0 2 Z\"/>"));
}

#[test]
fn contourless_glyphs_paint_nothing() {
    let tree = BoxTree::leaf(
        Metrics::new(3.0, 4.0, 1.0).unwrap(),
        Some(Bounds::new(0.0, 0.0, 0.0, 0.0).unwrap()),
    );
    let glyph = placed_glyph(Point::new(0.0, 4.0).unwrap(), None);
    let document = emit(&tree, &[glyph], &SvgOptions::default()).unwrap();
    assert!(!document.as_str().contains("<path"));
    assert!(!document.as_str().contains("glyphs"));
}

#[test]
fn multiple_glyphs_and_contours_keep_exact_paint_order() {
    let tree = BoxTree::space(Metrics::new(8.0, 8.0, 0.0).unwrap());
    let glyphs = [
        placed_glyph(
            Point::new(1.0, 4.0).unwrap(),
            Some(font_outline_px(vec![
                vec![point_px(0.0, 0.0, true), point_px(2.0, 2.0, true)],
                vec![point_px(0.5, 0.5, true), point_px(1.0, 1.0, true)],
            ])),
        ),
        placed_glyph(Point::default(), None),
        placed_glyph(
            Point::new(5.0, 6.0).unwrap(),
            Some(font_outline_px(vec![vec![
                point_px(0.0, 0.0, true),
                point_px(2.0, 1.0, true),
            ]])),
        ),
    ];
    let expected = concat!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8px\" height=\"8px\" viewBox=\"0 0 8 8\">",
        "<g fill=\"#000000\"><g id=\"box-0\" transform=\"translate(0 0)\"></g>",
        "<g id=\"glyphs\"><path d=\"M1 4 L3 2 ZM1.5 3.5 L2 3 Z\"/>",
        "<path d=\"M5 6 L7 5 Z\"/></g></g></svg>",
    );
    for _ in 0..8 {
        let document = emit(&tree, &glyphs, &SvgOptions::default()).unwrap();
        assert_eq!(document.as_str(), expected);
        parse(&document);
    }
}

#[test]
fn invalid_glyph_geometry_refuses_the_whole_document() {
    use logifos_svg::outline::OutlineError;

    let tree = BoxTree::space(Metrics::default());
    for (value, origin, expected) in [
        (f64::NAN, Point::default(), GeometryError::NonFinite),
        (f64::INFINITY, Point::default(), GeometryError::NonFinite),
        (
            f64::MAX,
            Point::new(f64::MAX, 0.0).unwrap(),
            GeometryError::Overflow,
        ),
    ] {
        let glyph = placed_glyph(
            origin,
            Some(font_outline_px(vec![vec![
                point_px(0.0, 0.0, true),
                point_px(value, 0.0, true),
            ]])),
        );
        let error = emit(&tree, &[glyph], &SvgOptions::default()).unwrap_err();
        assert_eq!(error, SvgError::Outline(OutlineError::Geometry(expected)));
        assert!(std::error::Error::source(&error).is_some());
    }
}

fn placed_glyph(
    origin: Point,
    outline: Option<logifos_layout::font::GlyphOutlinePx>,
) -> PlacedGlyph {
    PlacedGlyph {
        character: 'x',
        glyph: logifos_layout::font::ResolvedGlyph {
            resource_id: "test".into(),
            identity: logifos_layout::font::FontIdentity {
                sha256: [0; 32],
                face_index: 0,
            },
            glyph_id: 1,
            advance_px: 3.0,
            left_side_bearing_px: 0.0,
            italics_correction_px: 0.0,
            top_accent_px: 1.5,
            outline_px: outline,
        },
        origin,
        font_size_px: 16.0,
        color: None,
    }
}

fn font_outline_px(
    contours: Vec<Vec<logifos_layout::font::PointPx>>,
) -> logifos_layout::font::GlyphOutlinePx {
    logifos_layout::font::GlyphOutlinePx {
        contours: contours
            .into_iter()
            .map(|points| logifos_layout::font::ContourPx {
                points,
                cubic: false,
            })
            .collect(),
    }
}

fn point_px(x: f64, y: f64, on_curve: bool) -> logifos_layout::font::PointPx {
    logifos_layout::font::PointPx { x, y, on_curve }
}

#[test]
fn viewport_overflow_is_a_typed_geometry_error() {
    // Each layout subtree is valid. Including the parent's logical advance
    // alongside negative ink requires a width larger than binary64 permits.
    let tree = BoxTree::hbox(vec![
        kern(-f64::MAX),
        rule(f64::MAX, 1.0, 0.0).into(),
        kern(f64::MAX),
    ])
    .unwrap();
    let error = emit(&tree, &[], &SvgOptions::default()).unwrap_err();
    assert_eq!(error, SvgError::Geometry(GeometryError::Overflow));
    assert!(std::error::Error::source(&error).is_some());
    assert!(error.to_string().contains("viewport"));
}

#[test]
fn title_escapes_markup_entities_quotes_and_preserves_unicode() {
    let title = "α < β & \"quoted\" 'value' > 0 ]]> &amp;\r\n\t🦀";
    let options = SvgOptions {
        color: "#A1b2C3".into(),
        title: Some(title.into()),
    };
    let document = emit(&rule(1.0, 1.0, 0.0), &[], &options).unwrap();
    let parsed = parse(&document);
    assert_eq!(parsed.title.as_deref(), Some(title));
    assert!(document
        .as_str()
        .contains("&lt; β &amp; &quot;quoted&quot; &apos;value&apos; &gt;"));
    assert!(document.as_str().contains("fill=\"#a1b2c3\""));
    assert_eq!(options.color, "#A1b2C3");
}

#[test]
fn injected_title_remains_character_data_only() {
    let title = "</title><script>alert(1)</script><g onload=\"x\">https://example.test";
    let options = SvgOptions {
        title: Some(title.into()),
        ..SvgOptions::default()
    };
    let parsed = parse(&emit(&rule(1.0, 1.0, 0.0), &[], &options).unwrap());
    assert_eq!(parsed.title.as_deref(), Some(title));
    assert_eq!(
        parsed
            .elements
            .iter()
            .map(|e| e.name.as_str())
            .collect::<Vec<_>>(),
        ["svg", "title", "g", "g", "rect"]
    );
}

#[test]
fn invalid_colors_refuse_css_and_attribute_injection() {
    for color in [
        "",
        "red",
        "#fff",
        "#12345678",
        "#gg0000",
        " #123456",
        "#123456\n",
        "url(https://example.test)",
        "#000000\" onload=\"x",
        "#１２３",
    ] {
        let options = SvgOptions {
            color: color.into(),
            ..SvgOptions::default()
        };
        assert_eq!(
            emit(&rule(1.0, 1.0, 0.0), &[], &options),
            Err(SvgError::InvalidColor)
        );
    }
}

#[test]
fn invalid_xml_characters_refuse_instead_of_silent_replacement() {
    for value in [
        '\0', '\u{1}', '\u{8}', '\u{b}', '\u{c}', '\u{e}', '\u{1f}', '\u{fffe}', '\u{ffff}',
    ] {
        let options = SvgOptions {
            title: Some(format!("before{value}after")),
            ..SvgOptions::default()
        };
        assert_eq!(
            emit(&rule(1.0, 1.0, 0.0), &[], &options),
            Err(SvgError::InvalidXmlCharacter)
        );
    }
}

#[test]
fn numbers_round_trip_without_quantization_or_signed_zero() {
    for value in [
        f64::from_bits(1),
        f64::MIN_POSITIVE,
        0.000000123456789,
        1.2345678901234567,
        1e20,
        f64::MAX,
    ] {
        let document = emit_default(&rule(value, 1.0, 0.0));
        let parsed = parse(&document);
        assert_eq!(parsed.rectangles()[0][2].to_bits(), value.to_bits());
        let root_width: f64 = parsed.elements[0].attributes["width"]
            .strip_suffix("px")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(root_width.to_bits(), value.to_bits());
    }
    let tree = BoxTree::hbox(vec![kern(-0.0), rule(1.0, -0.0, 0.0).into()]).unwrap();
    let document = emit_default(&tree);
    for prefix in ["(-0", " -0", "=\"-0"] {
        assert!(!document.as_str().contains(prefix));
    }
    parse(&document).assert_viewport([0.0, 0.0, 1.0, 0.0]);
}

#[test]
fn repeated_emissions_have_no_cross_document_state() {
    let tree = BoxTree::hbox(vec![
        rule(2.0, 3.0, 0.0).into(),
        kern(-0.25),
        rule(1.0, 2.0, 1.0).into(),
    ])
    .unwrap();
    let options = SvgOptions {
        title: Some("repeatable".into()),
        ..SvgOptions::default()
    };
    let expected = emit(&tree, &[], &options).unwrap();
    for _ in 0..16 {
        emit_default(&rule(9.0, 8.0, 7.0));
        assert_eq!(
            emit(&tree.clone(), &[], &options.clone()).unwrap(),
            expected
        );
    }
    parse(&expected).assert_viewport([0.0, 0.0, 2.75, 4.0]);
}

#[test]
fn colored_glyphs_fill_their_own_path_only() {
    use logifos_layout::layout::MathColor;
    let outline = || {
        Some(font_outline_px(vec![vec![
            point_px(0.0, 0.0, true),
            point_px(2.0, 0.0, true),
            point_px(1.0, 2.0, true),
        ]]))
    };
    let plain = placed_glyph(Point::new(0.0, 2.0).unwrap(), outline());
    let red = PlacedGlyph {
        color: Some(MathColor::Red),
        ..placed_glyph(Point::new(4.0, 2.0).unwrap(), outline())
    };
    let tree = BoxTree::leaf(Metrics::new(6.0, 2.0, 0.0).unwrap(), None);
    let document = emit(&tree, &[plain, red], &SvgOptions::default()).unwrap();
    assert_eq!(
        document
            .as_str()
            .matches("<path fill=\"#ff0000\" d=")
            .count(),
        1
    );
    assert_eq!(document.as_str().matches("<path d=").count(), 1);
}
