#[allow(dead_code)]
mod support;

use logifos_layout::font::{FontErrorKind, FontFace, FontProvider, FontResource};
use logifos_layout::layout::{layout, Construct, Layout, LayoutError, LayoutOptions};
use logifos_layout::{BoxTree, Point};
use logifos_model::Formula;
use support::{digest, put16, put32, words, FontBuilder};

fn symbol(text: &str) -> Formula {
    Formula::Symbol(text.into())
}
fn operator(text: &str) -> Formula {
    Formula::Operator(text.into())
}
fn sequence(children: Vec<Formula>) -> Formula {
    Formula::Sequence(children)
}
fn options(level: u8) -> LayoutOptions {
    LayoutOptions {
        font_size_px: 18.0,
        script_level: level,
        display: false,
    }
}

// Fresh triangles with hand-selected bounds, advances and side bearings.
fn builder(characters: &[(char, u16)]) -> FontBuilder {
    let mut b = FontBuilder::new(characters, 250, false);
    put16(b.tables.get_mut(b"hhea").unwrap(), 34, 6);
    b.tables.insert(
        *b"hmtx",
        words(&[0, 0, 500, (-20i16) as u16, 700, 30, 400, 0, 300, 0, 200, 0]),
    );
    let mut glyf = Vec::new();
    let mut loca = vec![0; 28];
    for (id, width, bottom, top) in [
        (1, 400, -100i16, 600i16),
        (2, 600, -300, 800),
        (3, 350, 100, 500),
        (4, 250, -50, 350),
    ] {
        put32(&mut loca, id * 4, glyf.len() as u32);
        glyf.extend(words(&[1, 0, bottom as u16, width, top as u16, 2, 0]));
        glyf.extend([1, 1, 1]);
        glyf.extend(words(&[
            0,
            width,
            (-(width as i16)) as u16,
            bottom as u16,
            (top - bottom) as u16,
            (bottom - top) as u16,
        ]));
        glyf.push(0);
    }
    put32(&mut loca, 20, glyf.len() as u32);
    put32(&mut loca, 24, glyf.len() as u32);
    b.tables.insert(*b"glyf", glyf);
    b.tables.insert(*b"loca", loca);
    b
}
fn font() -> Vec<u8> {
    builder(&[
        ('x', 1),
        ('y', 2),
        ('z', 3),
        ('+', 4),
        ('-', 4),
        ('=', 4),
        ('*', 4),
        ('×', 4),
        (' ', 5),
    ])
    .finish()
}
fn provider(bytes: &[u8]) -> FontProvider {
    FontProvider::new(
        &[FontResource {
            id: "math",
            bytes,
            sha256: &digest(bytes),
            face_index: 0,
        }],
        &["math"],
    )
    .unwrap()
}
fn render(formula: &Formula, level: u8) -> Layout {
    layout(formula, &mut provider(&font()), options(level)).unwrap()
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-10,
        "actual {actual}, expected {expected}"
    );
}
fn invariants(tree: &BoxTree) {
    let m = tree.metrics();
    assert!(m.width().is_finite() && m.width() >= 0.0);
    assert!(m.height().is_finite() && m.height() >= 0.0);
    assert!(m.depth().is_finite() && m.depth() >= 0.0);
    for child in tree.children() {
        close(
            child.offset().y() + child.node().metrics().baseline(),
            m.baseline(),
        );
        invariants(child.node());
    }
}
fn snapshot(result: &Layout) -> String {
    fn visit(out: &mut String, tree: &BoxTree, depth: usize, offset: Point) {
        use std::fmt::Write;
        let m = tree.metrics();
        writeln!(
            out,
            "{}{:?} @ {:.3},{:.3} w/h/d {:.3}/{:.3}/{:.3}",
            "  ".repeat(depth),
            tree.kind(),
            offset.x(),
            offset.y(),
            m.width(),
            m.height(),
            m.depth()
        )
        .unwrap();
        if let Some(b) = tree.ink_bounds() {
            writeln!(
                out,
                "{}ink {:.3},{:.3} {:.3}x{:.3}",
                "  ".repeat(depth + 1),
                b.origin().x(),
                b.origin().y(),
                b.width(),
                b.height()
            )
            .unwrap();
        }
        for child in tree.children() {
            visit(out, child.node(), depth + 1, child.offset());
        }
    }
    use std::fmt::Write;
    let mut out = String::new();
    visit(&mut out, &result.tree, 0, Point::default());
    for g in &result.glyphs {
        writeln!(
            out,
            "glyph {:?} {}:{} @ {:.3},{:.3} size {:.3}",
            g.character,
            g.glyph.resource_id,
            g.glyph.glyph_id,
            g.origin.x(),
            g.origin.y(),
            g.font_size_px
        )
        .unwrap();
    }
    out
}

#[test]
fn mixed_row_snapshot_has_independently_calculated_geometry() {
    let formula = sequence(vec![
        symbol("x"),
        operator("+"),
        symbol("y"),
        operator("="),
        symbol("z"),
    ]);
    let result = render(&formula, 0);
    // At 18px: hmtx advances 9,5.4,12.6,5.4,7.2; margins 4+4+5+5;
    // the x-to-operator transition contributes 0.45px italic correction.
    close(result.tree.metrics().width(), 58.05);
    close(result.tree.metrics().height(), 14.4);
    close(result.tree.metrics().depth(), 5.4);
    for (g, x) in result.glyphs.iter().zip([0.0, 13.45, 22.85, 40.45, 50.85]) {
        close(g.origin.x(), x);
        close(g.origin.y(), 14.4);
    }
    let ink = result.tree.ink_bounds().unwrap();
    close(ink.origin().x(), -0.36);
    close(ink.origin().y(), 0.0);
    close(ink.width(), 57.51);
    close(ink.height(), 19.8);
    invariants(&result.tree);
    assert_eq!(snapshot(&result), include_str!("snapshots/mixed-row.txt"));
    assert_eq!(result, render(&formula, 0));
}

#[test]
fn core_spacing_categories_are_numeric_rules() {
    // Core 3.2.4.2, B.1: categories B (4mu), C (3mu), A (5mu).
    for (text, width, op_x, y_x) in [
        ("+", 35.45, 13.45, 22.85),
        ("-", 35.45, 13.45, 22.85),
        ("*", 33.45, 12.45, 20.85),
        ("×", 33.45, 12.45, 20.85),
        ("=", 37.45, 14.45, 24.85),
    ] {
        let result = render(&sequence(vec![symbol("x"), operator(text), symbol("y")]), 0);
        close(result.tree.metrics().width(), width);
        close(result.glyphs[1].origin.x(), op_x);
        close(result.glyphs[2].origin.x(), y_x);
    }
}

#[test]
fn inferred_forms_and_dictionary_fallback_are_distinct() {
    for text in ["+", "-"] {
        let prefix = render(&sequence(vec![operator(text), symbol("x")]), 0);
        close(prefix.glyphs[1].origin.x(), 5.4);
        close(prefix.tree.metrics().width(), 14.85);
        let postfix = render(&sequence(vec![symbol("y"), operator(text)]), 0);
        close(postfix.glyphs[1].origin.x(), 16.6);
        close(postfix.tree.metrics().width(), 26.0);
    }
    for (text, margin) in [("*", 3.0), ("×", 3.0), ("=", 5.0)] {
        let prefix = render(&sequence(vec![operator(text), symbol("y")]), 0);
        close(prefix.glyphs[0].origin.x(), margin);
        close(prefix.glyphs[1].origin.x(), 5.4 + 2.0 * margin);
    }
    let lone = render(&operator("+"), 0);
    close(lone.glyphs[0].origin.x(), 4.0);
    close(lone.tree.metrics().width(), 13.4);
}

#[test]
fn symbol_identity_never_infers_operator_spacing() {
    let result = render(&sequence(vec![symbol("y"), symbol("+"), symbol("y")]), 0);
    close(result.glyphs[1].origin.x(), 12.6);
    close(result.tree.metrics().width(), 30.6);
}

#[test]
fn single_operator_wrappers_apply_margins_once() {
    let wrapped = sequence(vec![
        sequence(vec![]),
        sequence(vec![operator("+")]),
        sequence(vec![]),
    ]);
    let result = render(&sequence(vec![symbol("x"), wrapped, symbol("y")]), 0);
    close(result.glyphs[1].origin.x(), 13.45);
    close(result.tree.metrics().width(), 35.45);
    let lone = render(&sequence(vec![sequence(vec![operator("+")])]), 0);
    close(lone.tree.metrics().width(), 13.4);
}

#[test]
fn nested_rows_preserve_local_form_and_baselines() {
    let result = render(
        &sequence(vec![
            symbol("y"),
            sequence(vec![operator("-"), symbol("z")]),
        ]),
        0,
    );
    close(result.glyphs[1].origin.x(), 12.6);
    close(result.glyphs[2].origin.x(), 18.0);
    close(result.tree.metrics().width(), 25.2);
    invariants(&result.tree);
}

#[test]
fn script_sizes_use_primary_percentages_then_core_deep_scaling() {
    let formula = sequence(vec![symbol("x"), operator("+"), symbol("y")]);
    for (level, size, width) in [
        (0, 18.0, 35.45),
        (1, 14.4, 28.36),
        (2, 10.8, 21.27),
        (3, 7.668, 15.1017),
    ] {
        let result = render(&formula, level);
        close(result.style.font_size_px, size);
        close(result.tree.metrics().width(), width);
        for glyph in &result.glyphs {
            close(glyph.font_size_px, size);
        }
        invariants(&result.tree);
    }
    assert_eq!(
        snapshot(&render(&formula, 2)),
        include_str!("snapshots/script-row.txt")
    );
}

#[test]
fn fallback_font_has_its_own_scale_bounds_and_identity() {
    let primary = font();
    let mut secondary = builder(&[('α', 1)]);
    put16(secondary.tables.get_mut(b"head").unwrap(), 18, 2000);
    put16(secondary.tables.get_mut(b"MATH").unwrap(), 10, 50);
    let secondary = secondary.finish();
    let mut fonts = FontProvider::new(
        &[
            FontResource {
                id: "primary",
                bytes: &primary,
                sha256: &digest(&primary),
                face_index: 0,
            },
            FontResource {
                id: "fallback",
                bytes: &secondary,
                sha256: &digest(&secondary),
                face_index: 0,
            },
        ],
        &["primary", "fallback"],
    )
    .unwrap();
    let result = layout(
        &sequence(vec![symbol("y"), symbol("α")]),
        &mut fonts,
        options(1),
    )
    .unwrap();
    // Primary's 80% gives 14.4px; fallback uses 2000 units/em, not 1000.
    close(result.style.font_size_px, 14.4);
    close(result.glyphs[1].glyph.advance_px, 3.6);
    close(result.tree.metrics().width(), 13.86); // 10.08 + 3.6 + 0.18
    close(result.tree.metrics().height(), 11.52);
    close(result.tree.metrics().depth(), 4.32);
    assert_eq!(
        result.glyphs[1].glyph.identity.sha256,
        FontFace::load(&secondary, 0).unwrap().identity.sha256
    );
    assert_eq!(fonts.fonts_used().len(), 2);
    assert_eq!(
        snapshot(&result),
        include_str!("snapshots/fallback-row.txt")
    );
    invariants(&result.tree);
}

#[test]
fn empty_and_contourless_tokens_keep_advance_without_fabricated_ink() {
    for formula in [symbol(""), sequence(vec![]), sequence(vec![symbol("")])] {
        let result = render(&formula, 0);
        assert_eq!(result.tree.metrics(), Default::default());
        assert_eq!(result.tree.ink_bounds(), None);
        assert!(result.glyphs.is_empty());
    }
    let result = render(&symbol(" "), 0);
    close(result.tree.metrics().width(), 3.6);
    close(result.tree.metrics().height(), 0.0);
    close(result.tree.metrics().depth(), 0.0);
    assert_eq!(result.tree.ink_bounds(), None);
}

#[test]
fn multicharacter_tokens_use_ordered_advances_and_last_italic_correction() {
    let result = render(&symbol("xyx"), 0);
    close(result.tree.metrics().width(), 31.05); // 9 + 12.6 + 9 + 0.45
    for (g, x) in result.glyphs.iter().zip([0.0, 9.0, 21.6]) {
        close(g.origin.x(), x);
    }
    invariants(&result.tree);
    let slanted = render(&sequence(vec![symbol("x"), symbol("x")]), 0);
    close(slanted.glyphs[1].origin.x(), 9.0);
    close(slanted.tree.metrics().width(), 18.45);
}

#[test]
fn unsupported_constructs_refuse_even_inside_rows() {
    for (node, kind) in [
        (
            Formula::Superscript(Box::new(symbol("x")), Box::new(symbol("y"))),
            Construct::Superscript,
        ),
        (
            Formula::Subscript(Box::new(symbol("x")), Box::new(symbol("y"))),
            Construct::Subscript,
        ),
    ] {
        for formula in [node.clone(), sequence(vec![symbol("x"), node])] {
            let error = layout(&formula, &mut provider(&font()), options(0)).unwrap_err();
            assert_eq!(error, LayoutError::UnsupportedConstruct(kind));
            assert_eq!(error.code(), "NOT_IMPLEMENTED");
        }
    }
}

#[test]
fn fraction_lays_out_its_children_around_the_axis_rule() {
    use logifos_layout::font::math::MathConstant;

    let mut fonts = provider(&font());
    let scale = 18.0 / 1000.0;
    let mut unit =
        |constant: MathConstant| f64::from(fonts.primary().math.constants.value(constant)) * scale;
    // Fixture constants: 100 + spec index design units.
    let rule = unit(MathConstant::FractionRuleThickness);
    let gap_numerator = unit(MathConstant::FractionNumeratorGapMin);
    let gap_denominator = unit(MathConstant::FractionDenominatorGapMin);

    let formula = Formula::Fraction(Box::new(symbol("x")), Box::new(symbol("y")));
    let result = layout(&formula, &mut fonts, options(0)).unwrap();
    // 'x': box 9.0 wide, 10.8 tall, 1.8 deep; 'y': box 12.6 wide, 14.4 tall,
    // 5.4 deep (hmtx advances and triangle bounds at 18px over 1000upem).
    let (numerator_height, numerator_depth, numerator_width): (f64, f64, f64) = (10.8, 1.8, 9.0);
    let (denominator_height, denominator_depth, denominator_width): (f64, f64, f64) =
        (14.4, 5.4, 12.6);
    // The gap dominates the fixture's shift constants, so the shifts grow to
    // keep the minimum gaps.
    let shift_up = numerator_depth + rule / 2.0 + gap_numerator;
    let shift_down = denominator_height + rule / 2.0 + gap_denominator;
    let width = numerator_width.max(denominator_width);
    let metrics = result.tree.metrics();
    close(metrics.width(), width);
    close(metrics.baseline(), numerator_height + shift_up);
    close(metrics.depth(), shift_down + denominator_depth);

    // finish() wraps the vbox in an hbox; the fraction vbox is its only child.
    let children = result.tree.children();
    assert_eq!(children.len(), 1);
    let vbox = children[0].node();
    let stack = vbox.children();
    assert_eq!(stack.len(), 3);
    // The vbox stacks without overlap: the denominator's bottom is the root's.
    close(
        stack[2].offset().y() + denominator_height + denominator_depth,
        metrics.baseline() + metrics.depth(),
    );
    let rule_position = numerator_height + numerator_depth + gap_numerator;
    close(stack[1].offset().y(), rule_position);
    // The rule is centered on the fraction baseline: its top sits t/2 above it.
    close(stack[1].offset().y(), metrics.baseline() - rule / 2.0);
    close(stack[1].node().metrics().width(), width);
    close(stack[1].node().metrics().baseline(), rule);

    // Numerator glyphs keep their node-local origin (its top is the root top);
    // denominator glyphs translate by the denominator's stack offset.
    // Numerator glyphs keep their node-local origin (its top is the root top);
    // denominator glyphs translate by the denominator's stack offset.
    close(result.glyphs[0].origin.x(), (width - numerator_width) / 2.0);
    close(result.glyphs[0].origin.y(), numerator_height);
    close(
        result.glyphs[1].origin.x(),
        (width - denominator_width) / 2.0,
    );
    let denominator_offset = rule_position + rule + gap_denominator;
    close(
        result.glyphs[1].origin.y(),
        denominator_offset + denominator_height,
    );
    // Both children center in the fraction width.
    close(stack[0].node().metrics().width(), width);
    close(stack[2].node().metrics().width(), width);
}

#[test]
fn display_fractions_select_the_display_constants() {
    use logifos_layout::font::math::MathConstant;

    let mut fonts = provider(&font());
    let scale = 18.0 / 1000.0;
    let inline = layout(
        &Formula::Fraction(Box::new(symbol("x")), Box::new(symbol("y"))),
        &mut fonts,
        options(0),
    )
    .unwrap();
    let display = layout(
        &Formula::Fraction(Box::new(symbol("x")), Box::new(symbol("y"))),
        &mut fonts,
        LayoutOptions {
            font_size_px: 18.0,
            script_level: 0,
            display: true,
        },
    )
    .unwrap();
    // The gap constants dominate the fixture's shifts, so switching styles
    // changes exactly the gap: FractionNumDisplayStyleGapMin (133) and
    // FractionDenomDisplayStyleGapMin (136) versus 132 and 135.
    let height_delta = unit_of(&mut fonts, MathConstant::FractionNumDisplayStyleGapMin)
        - unit_of(&mut fonts, MathConstant::FractionNumeratorGapMin);
    close(
        display.tree.metrics().baseline() - inline.tree.metrics().baseline(),
        height_delta * scale,
    );
    close(height_delta, 1.0);
    close(
        display.tree.metrics().depth() - inline.tree.metrics().depth(),
        (136.0 - 135.0) * scale,
    );
    close(
        display.tree.children()[0].node().children()[1].offset().y(),
        display.tree.metrics().baseline()
            - unit_of(&mut fonts, MathConstant::FractionRuleThickness) * scale / 2.0,
    );
}

fn unit_of(fonts: &mut FontProvider, constant: logifos_layout::font::math::MathConstant) -> f64 {
    f64::from(fonts.primary().math.constants.value(constant))
}

#[test]
fn unsupported_operators_and_missing_glyphs_refuse_typed() {
    for text in ["", "∑", "++", "−"] {
        assert_eq!(
            layout(&operator(text), &mut provider(&font()), options(0)).unwrap_err(),
            LayoutError::UnsupportedOperator(text.into())
        );
    }
    let error = layout(&symbol("q"), &mut provider(&font()), options(0)).unwrap_err();
    assert!(matches!(error,LayoutError::Font(ref e) if e.kind == FontErrorKind::MissingGlyph));
    assert_eq!(error.code(), "MISSING_GLYPH");
    // A cmap entry resolving to .notdef does not qualify as coverage.
    let bytes = builder(&[('q', 0)]).finish();
    assert_eq!(
        provider(&bytes).resolve('q', 18.0).unwrap_err().kind,
        FontErrorKind::MissingGlyph
    );
}

#[test]
fn invalid_sizes_and_script_constants_refuse_typed() {
    for size in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -1.0,
        -0.0,
        0.0,
        4096.1,
    ] {
        assert_eq!(
            layout(
                &symbol("x"),
                &mut provider(&font()),
                LayoutOptions {
                    font_size_px: size,
                    script_level: 0,
                    display: false
                }
            )
            .unwrap_err(),
            LayoutError::InvalidOptions
        );
    }
    for (first, second, level) in [(0, 60, 1), (101, 60, 1), (80, 0, 2), (80, 90, 2)] {
        let mut b = builder(&[('x', 1)]);
        let math = b.tables.get_mut(b"MATH").unwrap();
        put16(math, 10, first);
        put16(math, 12, second);
        assert_eq!(
            layout(&symbol("x"), &mut provider(&b.finish()), options(level)).unwrap_err(),
            LayoutError::InvalidMathConstants
        );
    }
    assert_eq!(
        layout(
            &symbol("x"),
            &mut provider(&font()),
            LayoutOptions {
                font_size_px: f64::from_bits(1),
                script_level: 255,
                display: false
            }
        )
        .unwrap_err(),
        LayoutError::InvalidOptions
    );
}

#[test]
fn missing_math_and_malformed_glyph_metrics_never_become_guesses() {
    let mut b = builder(&[('x', 1)]);
    b.tables.remove(b"MATH");
    let bytes = b.finish();
    assert_eq!(
        FontFace::load(&bytes, 0).unwrap_err().kind,
        FontErrorKind::MissingTable
    );
    let mut b = builder(&[('x', 1)]);
    put16(b.tables.get_mut(b"glyf").unwrap(), 2, 1000); // xMin > xMax
    assert_eq!(
        FontFace::load(&b.finish(), 0).unwrap_err().kind,
        FontErrorKind::MalformedTable
    );
    let mut b = builder(&[('x', 1)]);
    put32(b.tables.get_mut(b"loca").unwrap(), 8, 4); // nonempty, truncated header
    assert_eq!(
        FontFace::load(&b.finish(), 0).unwrap_err().kind,
        FontErrorKind::MalformedTable
    );
}

#[test]
fn direct_formula_values_are_bounded() {
    let mut nested = symbol("x");
    for _ in 0..logifos_model::MAX_DEPTH {
        nested = sequence(vec![nested]);
    }
    assert_eq!(
        layout(&nested, &mut provider(&font()), options(0)).unwrap_err(),
        LayoutError::ResourceLimit
    );
    let too_many = sequence(vec![symbol(""); logifos_model::MAX_NODES]);
    assert_eq!(
        layout(&too_many, &mut provider(&font()), options(0)).unwrap_err(),
        LayoutError::ResourceLimit
    );
    let too_long = symbol(&"y".repeat(logifos_model::MAX_NODES + 1));
    assert_eq!(
        layout(&too_long, &mut provider(&font()), options(0)).unwrap_err(),
        LayoutError::ResourceLimit
    );
}

#[test]
fn empty_subtrees_do_not_consume_a_trailing_italic_correction() {
    let result = render(
        &sequence(vec![
            symbol("x"),
            sequence(vec![]),
            symbol(""),
            symbol("x"),
            sequence(vec![]),
        ]),
        0,
    );
    close(result.glyphs[1].origin.x(), 9.0);
    close(result.tree.metrics().width(), 18.45);
}

fn cff_font() -> Vec<u8> {
    let mut b = builder(&[('x', 1)]);
    b.tables.remove(b"glyf");
    b.tables.remove(b"loca");
    // CFF 1 header; one Name INDEX; one Top DICT with CharStrings at byte 21;
    // empty String and Global Subr INDEXes; six endchar programs.
    let mut cff = vec![
        1, 0, 4, 4, 0, 1, 1, 1, 2, b'T', 0, 1, 1, 1, 3, 160, 17, 0, 0, 0, 0,
    ];
    cff.extend([0, 6, 1, 1, 2, 3, 4, 5, 6, 7]);
    cff.extend([14; 6]);
    b.tables.insert(*b"CFF ", cff);
    b.tables.insert(*b"maxp", vec![0, 0, 0x50, 0, 0, 6]);
    let mut bytes = b.finish();
    bytes[..4].copy_from_slice(b"OTTO");
    let head = (0..support::get16(&bytes, 4))
        .find_map(|i| {
            let at = 12 + i * 16;
            (&bytes[at..at + 4] == b"head").then(|| support::get32(&bytes, at + 8))
        })
        .unwrap();
    put32(&mut bytes, head + 8, 0);
    let sum = bytes.as_chunks::<4>().0.iter().fold(0u32, |sum, chunk| {
        sum.wrapping_add(u32::from_be_bytes(*chunk))
    });
    put32(&mut bytes, head + 8, 0xb1b0_afbau32.wrapping_sub(sum));
    bytes
}

#[test]
fn missing_cff_bounds_refuse_without_skipping_the_first_covering_font() {
    let cff = cff_font();
    let ttf = font();
    let mut fonts = FontProvider::new(
        &[
            FontResource {
                id: "compact",
                bytes: &cff,
                sha256: &digest(&cff),
                face_index: 0,
            },
            FontResource {
                id: "truetype",
                bytes: &ttf,
                sha256: &digest(&ttf),
                face_index: 0,
            },
        ],
        &["compact", "truetype"],
    )
    .unwrap();
    assert_eq!(fonts.resolve('x', 18.0).unwrap().resource_id, "compact");
    let error = layout(&symbol("x"), &mut fonts, options(0)).unwrap_err();
    assert!(
        matches!(error,LayoutError::Font(ref e) if e.kind == FontErrorKind::MetricsUnavailable)
    );
    assert_eq!(error.code(), "MISSING_METRICS");
    assert_eq!(fonts.fonts_used().len(), 1);
}

#[test]
fn signed_italic_correction_is_retained_and_applied_once() {
    let mut b = builder(&[('x', 1), ('y', 2)]);
    let math = b.tables.get_mut(b"MATH").unwrap();
    let info = support::get16(math, 6);
    let italics = info + support::get16(math, info);
    put16(math, italics + 4, (-25i16) as u16);
    let result = layout(
        &sequence(vec![symbol("x"), symbol("y")]),
        &mut provider(&b.finish()),
        options(0),
    )
    .unwrap();
    close(result.glyphs[1].origin.x(), 8.55);
    close(result.tree.metrics().width(), 21.15);
}
