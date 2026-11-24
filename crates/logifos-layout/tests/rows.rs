#[allow(dead_code)]
mod support;

use logifos_layout::font::{FontErrorKind, FontFace, FontProvider, FontResource};
use logifos_layout::layout::{layout, Layout, LayoutError, LayoutOptions};
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
/// The support builder's font with stretch data: vertical constructions for
/// glyph 2 ('√') — variants g2 (800u) and g3 (1200u) plus a three-part
/// assembly g3/g4(extender)/g5 — and every glyph a 600u-advance triangle
/// spanning −200..700 units. `x_top` overrides glyph 1's ink top.
fn stretch_font(x_top: i16) -> Vec<u8> {
    let mut b = support::FontBuilder::new(
        &[('x', 1), ('√', 2), ('(', 2), (')', 2), ('z', 3)],
        250,
        true,
    );
    put16(b.tables.get_mut(b"glyf").unwrap(), 8, x_top as u16);
    b.finish()
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
fn superscript_lays_out_the_script_above_the_base_per_the_shift_rules() {
    use logifos_layout::font::math::MathConstant;

    let mut fonts = provider(&font());
    let scale = 18.0 / 1000.0;
    let mut unit =
        |constant: MathConstant| f64::from(fonts.primary().math.constants.value(constant)) * scale;
    // SuperShift = max(ShiftUp, BottomMin + script ink descent,
    //                  base ink ascent - BaselineDropMax): the drop dominates.
    let shift = (10.8 - unit(MathConstant::SuperscriptBaselineDropMax))
        .max(unit(MathConstant::SuperscriptBottomMin) + 4.32)
        .max(unit(MathConstant::SuperscriptShiftUp));
    close(shift, 8.82);
    let width = 9.0 + 0.45 + 10.08 + unit(MathConstant::SpaceAfterScript);
    let formula = Formula::Superscript(Box::new(symbol("x")), Box::new(symbol("y")));
    let result = layout(&formula, &mut fonts, options(0)).unwrap();
    // Base 'x': 9.0 wide with ink ascent 10.8 and 0.45 italic correction.
    // Script 'y' builds at level one (80%): 10.08 wide, 11.52 tall, 4.32 deep.
    let metrics = result.tree.metrics();
    // finish() appends the construct's carried correction once more, as the
    // row's trailing italic advance for whatever follows.
    close(metrics.width(), width + 0.45);
    close(metrics.baseline(), 11.52 + shift);
    close(metrics.depth(), 1.8);
    assert_eq!(result.glyphs.len(), 2);
    close(result.glyphs[0].font_size_px, 18.0);
    close(result.glyphs[1].font_size_px, 14.4);
    // The base stays on the row baseline; the script's baseline rises by
    // exactly the shift, and the correction kern separates them.
    close(result.glyphs[0].origin.x(), 0.0);
    close(result.glyphs[0].origin.y(), metrics.baseline());
    close(result.glyphs[1].origin.x(), 9.0 + 0.45);
    close(result.glyphs[1].origin.y(), metrics.baseline() - shift);
    // The script wrapper's raised baseline carries the shift; its trailing
    // pad keeps the wrapper's extent below its own baseline.
    let row = result.tree.children()[0].node();
    let wrapper = row.children()[1].node();
    assert_eq!(wrapper.kind(), logifos_layout::BoxKind::VBox);
    close(wrapper.metrics().baseline(), 11.52 + shift);
    assert_eq!(
        snapshot(&result),
        include_str!("snapshots/superscript-row.txt")
    );
    assert_eq!(result, layout(&formula, &mut fonts, options(0)).unwrap());
}

#[test]
fn subscript_lays_out_the_script_below_the_base_per_the_shift_rules() {
    use logifos_layout::font::math::MathConstant;

    let mut fonts = provider(&font());
    let scale = 18.0 / 1000.0;
    let mut unit =
        |constant: MathConstant| f64::from(fonts.primary().math.constants.value(constant)) * scale;
    // SubShift = max(ShiftDown, script ink ascent - TopMax,
    //                BaselineDropMin + base ink descent): the ascent clamp wins.
    let shift = unit(MathConstant::SubscriptShiftDown)
        .max(11.52 - unit(MathConstant::SubscriptTopMax))
        .max(unit(MathConstant::SubscriptBaselineDropMin) + 1.8);
    close(shift, 9.63);
    let width = 9.0 + 10.08 + unit(MathConstant::SpaceAfterScript);
    let formula = Formula::Subscript(Box::new(symbol("x")), Box::new(symbol("y")));
    let result = layout(&formula, &mut fonts, options(0)).unwrap();
    // Script 'y' builds at level one (80%): 10.08 wide, 11.52 tall, 4.32 deep.
    let metrics = result.tree.metrics();
    close(metrics.width(), width);
    close(metrics.baseline(), 10.8);
    close(metrics.depth(), shift + 4.32);
    // No correction kern for subscripts: the script follows the base box.
    close(result.glyphs[1].origin.x(), 9.0);
    close(result.glyphs[1].origin.y(), metrics.baseline() + shift);
    // The wrapper's lowered baseline leaves the base's height as the row's.
    let row = result.tree.children()[0].node();
    let wrapper = row.children()[1].node();
    assert_eq!(wrapper.kind(), logifos_layout::BoxKind::VBox);
    close(wrapper.metrics().baseline(), 11.52 - shift);
    assert_eq!(
        snapshot(&result),
        include_str!("snapshots/subscript-row.txt")
    );
    assert_eq!(result, layout(&formula, &mut fonts, options(0)).unwrap());
}

#[test]
fn script_children_raise_one_level_and_keep_compact_display() {
    let fraction = || Formula::Fraction(Box::new(symbol("x")), Box::new(symbol("y")));
    let mut fonts = provider(&font());
    let direct = layout(&fraction(), &mut fonts, options(1)).unwrap();
    let inside = layout(
        &Formula::Superscript(Box::new(symbol("x")), Box::new(fraction())),
        &mut fonts,
        options(0),
    )
    .unwrap();
    // The scripted fraction is one level deeper than its base, at the same
    // size the direct level-one call resolves to.
    close(inside.glyphs[0].font_size_px, 18.0);
    for glyph in &inside.glyphs[1..] {
        // Compact fraction children gain a further level (60% at depth two).
        close(glyph.font_size_px, 10.8);
    }
    // Compact displaystyle: the in-script fraction must equal the direct
    // level-one fraction (which options(1) lays out compact) byte for byte.
    let wrapper = inside.tree.children()[0].node().children()[1].node();
    assert_eq!(
        wrapper.children()[0].node(),
        direct.tree.children()[0].node()
    );
}

#[test]
fn nested_scripts_use_the_cramped_superscript_constant() {
    use logifos_layout::font::math::MathConstant;

    // A low base and a high-floating script leave the plain shift constant
    // in charge, so cramped versus relaxed is the only change in the max().
    let mut b = builder(&[('x', 1), ('z', 3)]);
    put16(b.tables.get_mut(b"glyf").unwrap(), 8, 200); // 'x' ink top: 200 units
    let bytes = b.finish();
    let mut fonts = provider(&bytes);
    let value = |fonts: &mut FontProvider, constant: MathConstant, size: f64| -> f64 {
        f64::from(fonts.primary().math.constants.value(constant)) * size / 1000.0
    };
    // Relaxed top level: SuperscriptShiftUp wins the max() at 18px, and the
    // 'z' script is 7.2 tall at 14.4px.
    let relaxed = layout(
        &Formula::Superscript(Box::new(symbol("x")), Box::new(symbol("z"))),
        &mut fonts,
        options(0),
    )
    .unwrap();
    close(
        relaxed.tree.metrics().baseline(),
        7.2 + value(&mut fonts, MathConstant::SuperscriptShiftUp, 18.0),
    );
    // Inside a subscript the same construct is cramped and one level up:
    // the inner wrapper's raised baseline carries SuperscriptShiftUpCramped
    // at 14.4px over the 5.4-tall 'z' at 10.8px.
    let nested = layout(
        &Formula::Subscript(
            Box::new(symbol("x")),
            Box::new(Formula::Superscript(
                Box::new(symbol("x")),
                Box::new(symbol("z")),
            )),
        ),
        &mut fonts,
        options(0),
    )
    .unwrap();
    for (glyph, size) in nested.glyphs.iter().zip([18.0, 14.4, 10.8]) {
        close(glyph.font_size_px, size);
    }
    let inner_wrapper = nested.tree.children()[0].node().children()[1]
        .node()
        .children()[0]
        .node()
        .children()[1]
        .node();
    close(
        inner_wrapper.metrics().baseline(),
        5.4 + value(&mut fonts, MathConstant::SuperscriptShiftUpCramped, 14.4),
    );
    close(inner_wrapper.metrics().baseline(), 6.9552);
}

#[test]
fn sqrt_selects_a_covering_variant_and_wraps_the_radicand() {
    use logifos_layout::font::math::MathConstant;

    let mut fonts = provider(&stretch_font(700));
    let scale = 18.0 / 1000.0;
    let mut unit =
        |constant: MathConstant| f64::from(fonts.primary().math.constants.value(constant)) * scale;
    let gap = unit(MathConstant::RadicalVerticalGap);
    let rule = unit(MathConstant::RadicalRuleThickness);
    let extra = unit(MathConstant::RadicalExtraAscender);
    // 'x': advance 10.8 with ink 12.6 above and 3.6 below the baseline; the
    // target 21.456px is covered by the g3 variant (1200u = 21.6px).
    let ascent = 12.6f64.max(12.6 + gap + rule + extra);
    let extent = 1200.0 * scale;
    let formula = Formula::Sqrt(Box::new(symbol("x")));
    let result = layout(&formula, &mut fonts, options(0)).unwrap();
    let metrics = result.tree.metrics();
    close(metrics.width(), 21.6);
    close(metrics.baseline(), ascent);
    close(metrics.depth(), extent + extra - ascent);
    assert_eq!(result.glyphs.len(), 2);
    assert_eq!(result.glyphs[0].glyph.glyph_id, 3);
    close(result.glyphs[0].font_size_px, 18.0);
    // The surd piece hangs below the extra ascender with its baseline at the
    // slot bottom; the radicand keeps the row baseline after the surd.
    close(result.glyphs[0].origin.x(), 0.0);
    close(result.glyphs[0].origin.y(), extra + extent);
    close(result.glyphs[1].origin.x(), 10.8);
    close(result.glyphs[1].origin.y(), ascent);
    // The overbar rides the radicand width, centered rule thicknesses below
    // the content top; its zero advance keeps the row at surd + radicand.
    let wrapper = result.tree.children()[0].node().children()[1].node();
    close(wrapper.metrics().baseline(), ascent - extra);
    close(wrapper.metrics().width(), 10.8);
    assert_eq!(snapshot(&result), include_str!("snapshots/sqrt-row.txt"));
    assert_eq!(result, layout(&formula, &mut fonts, options(0)).unwrap());
}

#[test]
fn sqrt_grows_the_assembly_extender_to_the_target() {
    use logifos_layout::font::math::MathConstant;

    let mut fonts = provider(&stretch_font(1400));
    let scale = 18.0 / 1000.0;
    let mut unit =
        |constant: MathConstant| f64::from(fonts.primary().math.constants.value(constant)) * scale;
    let gap = unit(MathConstant::RadicalVerticalGap);
    let rule = unit(MathConstant::RadicalRuleThickness);
    let extra = unit(MathConstant::RadicalExtraAscender);
    // 'x' ink top 1400: ascent 25.2, descent 3.6; the target 34.056px exceeds
    // the natural 1600u assembly, so the 600u extender repeats
    // ceil((target - 1000u)/600u) = 2 times: parts g3, g4, g4, g5.
    let ascent = 25.2f64.max(25.2 + gap + rule + extra);
    let target = rule + gap + 28.8;
    let extent = 2200.0 * scale; // 39.6: 500 + 2x600 + 500 units
    let overlap = (extent - target) / 3.0;
    // The stretched box spans its slots' union, not the raw part total.
    let stretched = extent - 3.0 * overlap;
    let formula = Formula::Sqrt(Box::new(symbol("x")));
    let result = layout(&formula, &mut fonts, options(0)).unwrap();
    let metrics = result.tree.metrics();
    close(metrics.width(), 21.6);
    close(metrics.baseline(), ascent);
    close(metrics.depth(), stretched + extra - ascent);
    assert_eq!(result.glyphs.len(), 5);
    for (glyph, id) in result.glyphs.iter().zip([3u16, 4, 4, 5, 1]) {
        assert_eq!(glyph.glyph.glyph_id, id);
    }
    // Piece baselines stack at slot bottoms: each slot grows by its extent
    // less the shared overlap, starting under the extra ascender.
    let mut slot = 0.0;
    for glyph in &result.glyphs[..4] {
        let piece_extent = if glyph.glyph.glyph_id == 4 { 10.8 } else { 9.0 };
        close(glyph.origin.y(), extra + slot + piece_extent);
        slot += piece_extent - overlap;
    }
    close(result.glyphs[4].origin.x(), 10.8);
    close(result.glyphs[4].origin.y(), ascent);
    assert_eq!(
        snapshot(&result),
        include_str!("snapshots/sqrt-assembly-row.txt")
    );
    assert_eq!(result, layout(&formula, &mut fonts, options(0)).unwrap());
}

#[test]
fn root_raises_the_index_by_the_percent_schedule() {
    use logifos_layout::font::math::MathConstant;

    let mut fonts = provider(&stretch_font(700));
    let scale = 18.0 / 1000.0;
    let mut unit =
        |constant: MathConstant| f64::from(fonts.primary().math.constants.value(constant)) * scale;
    let gap = unit(MathConstant::RadicalVerticalGap);
    let rule = unit(MathConstant::RadicalRuleThickness);
    let extra = unit(MathConstant::RadicalExtraAscender);
    let kern_before = unit(MathConstant::RadicalKernBeforeDegree);
    // Index 'z' at level two (10.8px): 6.48 wide, 7.56 tall, 2.16 deep.
    let index_ascent = 7.56;
    let index_width = 6.48;
    // The fixture overrides the last constant record: KernAfterDegree -120u,
    // which the max(-index width) clamp keeps.
    let kern_after = unit(MathConstant::RadicalKernAfterDegree).max(-index_width);
    let percent = f64::from(
        fonts
            .primary()
            .math
            .constants
            .radical_degree_bottom_raise_percent,
    ) / 100.0;
    // B: the sqrt row of 'x' — width 21.6, ascent 20.52, depth 3.744.
    let b_ascent = 12.6f64.max(12.6 + gap + rule + extra);
    let b_depth = 1200.0 * scale + extra - b_ascent;
    // The index top rides -B.depth + percent x B.block above the baseline.
    let index_top = -b_depth + percent * (b_ascent + b_depth) + index_ascent;
    let formula = Formula::Root(Box::new(symbol("x")), Box::new(symbol("z")));
    let result = layout(&formula, &mut fonts, options(0)).unwrap();
    let metrics = result.tree.metrics();
    close(
        metrics.width(),
        kern_before + index_width + kern_after + 21.6,
    );
    close(metrics.baseline(), index_top.max(b_ascent));
    // The spec descent formula loses to B's own depth in this fixture.
    close(
        metrics.depth(),
        (b_depth - percent * (b_ascent + b_depth) + index_ascent)
            .max(b_depth)
            .max(1200.0 * scale + extra - b_ascent),
    );
    assert_eq!(result.glyphs.len(), 3);
    for (glyph, id) in result.glyphs.iter().zip([3u16, 3, 1]) {
        assert_eq!(glyph.glyph.glyph_id, id);
    }
    for (glyph, size) in result.glyphs.iter().zip([10.8, 18.0, 18.0]) {
        close(glyph.font_size_px, size);
    }
    // The index wrapper's raised baseline: top minus one index height.
    let wrapper_offset_y = metrics.baseline() - index_top;
    close(result.glyphs[0].origin.x(), kern_before);
    close(result.glyphs[0].origin.y(), wrapper_offset_y + index_ascent);
    let b_x = kern_before + index_width + kern_after;
    close(result.glyphs[1].origin.x(), b_x);
    close(result.glyphs[1].origin.y(), extra + 1200.0 * scale);
    close(result.glyphs[2].origin.x(), b_x + 10.8);
    close(result.glyphs[2].origin.y(), metrics.baseline());
    assert_eq!(snapshot(&result), include_str!("snapshots/root-row.txt"));
    assert_eq!(result, layout(&formula, &mut fonts, options(0)).unwrap());
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
    // Compact fractions raise child depth: 18px * 80% = 14.4px.
    let (numerator_height, numerator_depth, numerator_width): (f64, f64, f64) = (8.64, 1.44, 7.2);
    let (denominator_height, denominator_depth, denominator_width): (f64, f64, f64) =
        (11.52, 4.32, 10.08);
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
    // Display fractions retain level-zero child size; compact fractions use
    // 80%. The gap constants also change from 132/135 to 133/136.
    let height_delta = unit_of(&mut fonts, MathConstant::FractionNumDisplayStyleGapMin)
        - unit_of(&mut fonts, MathConstant::FractionNumeratorGapMin);
    close(
        display.tree.metrics().baseline() - inline.tree.metrics().baseline(),
        height_delta * scale + (10.8 + 1.8) * 0.2,
    );
    close(height_delta, 1.0);
    close(
        display.tree.metrics().depth() - inline.tree.metrics().depth(),
        (136.0 - 135.0) * scale + (14.4 + 5.4) * 0.2,
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

#[test]
fn tables_center_cells_on_the_math_axis_with_mtd_padding() {
    use logifos_layout::font::math::MathConstant;

    let mut fonts = provider(&font());
    let scale = 18.0 / 1000.0;
    let mut unit =
        |constant: MathConstant| f64::from(fonts.primary().math.constants.value(constant)) * scale;
    let axis = unit(MathConstant::AxisHeight);
    // mtd padding: 0.4em horizontal, 0.5ex vertical over OS/2 sxHeight 450
    // (ex = 450 units = 8.1px at 18px).
    let horizontal_pad = 7.2;
    let vertical_pad = 4.05;
    let formula = Formula::Table(vec![
        vec![
            Formula::Sequence(vec![symbol("x")]),
            Formula::Sequence(vec![symbol("y")]),
        ],
        vec![
            Formula::Sequence(vec![symbol("y")]),
            Formula::Sequence(vec![symbol("x")]),
        ],
    ]);
    let result = layout(&formula, &mut fonts, options(0)).unwrap();
    let metrics = result.tree.metrics();
    // Padded widths: x 9 + 14.4 = 23.4, y 12.6 + 14.4 = 27.0; both columns
    // take the larger 27.0, so the grid is 54 wide.
    close(metrics.width(), 54.0);
    // Padded rows: height max(10.8, 14.4) + 4.05 = 18.45 above the baseline;
    // depth max(1.8, 5.4) + 4.05 = 9.45 below it.
    let row_height = 14.4 + vertical_pad;
    let row_depth = 5.4 + vertical_pad;
    let total = 2.0 * (row_height + row_depth);
    close(metrics.baseline(), total / 2.0 + axis);
    close(metrics.depth(), total - (total / 2.0 + axis));
    // Cells baseline-align within each row and center in their column.
    // Row 0: x then y; row 1: y then x.
    let row_baseline_1 = row_height + row_depth;
    close(
        result.glyphs[0].origin.x(),
        (27.0 - 23.4) / 2.0 + horizontal_pad,
    );
    close(result.glyphs[0].origin.y(), row_height);
    close(result.glyphs[1].origin.x(), 27.0 + horizontal_pad);
    close(result.glyphs[1].origin.y(), row_height);
    close(result.glyphs[2].origin.x(), horizontal_pad);
    close(result.glyphs[2].origin.y(), row_baseline_1 + row_height);
    close(
        result.glyphs[3].origin.x(),
        27.0 + (27.0 - 23.4) / 2.0 + horizontal_pad,
    );
    close(result.glyphs[3].origin.y(), row_baseline_1 + row_height);
    // The math-style compact rule: a fraction in a cell must equal the
    // direct non-display level-zero fraction.
    let mut fonts = provider(&font());
    let direct = layout(
        &Formula::Fraction(Box::new(symbol("x")), Box::new(symbol("y"))),
        &mut fonts,
        options(0),
    )
    .unwrap();
    let fraction = Formula::Fraction(Box::new(symbol("x")), Box::new(symbol("y")));
    let inside = layout(
        &Formula::Table(vec![vec![Formula::Sequence(vec![fraction])]]),
        &mut fonts,
        options(0),
    )
    .unwrap();
    // finish hbox > table vbox > row hbox > padded wrapper > the cell's own
    // row hbox > the fraction's own tree.
    let cell_box = inside.tree.children()[0].node().children()[0]
        .node()
        .children()[0]
        .node()
        .children()[0]
        .node()
        .children()[0]
        .node();
    assert_eq!(cell_box, direct.tree.children()[0].node());
    assert_eq!(snapshot(&result), include_str!("snapshots/table-grid.txt"));
    let mut fonts = provider(&font());
    assert_eq!(result, layout(&formula, &mut fonts, options(0)).unwrap());
}

#[test]
fn fences_stretch_to_the_row_ink_extents() {
    let mut fonts = provider(&stretch_font(700));
    let formula = sequence(vec![operator("("), symbol("x"), operator(")")]);
    let result = layout(&formula, &mut fonts, options(0)).unwrap();
    // 'x' (600u advance, ink 12.6 above and 3.6 below the baseline) makes
    // the fences re-lay at a 16.2px target, which the g3 variant (21.6px)
    // covers; all three children carry the 600u advance of 10.8px.
    let metrics = result.tree.metrics();
    close(metrics.width(), 10.8 + 10.8 + 0.45 + 10.8);
    close(metrics.height(), 12.6);
    close(metrics.depth(), 3.6);
    assert_eq!(result.glyphs.len(), 3);
    for glyph in [
        result.glyphs[0].glyph.glyph_id,
        result.glyphs[2].glyph.glyph_id,
    ] {
        assert_eq!(glyph, 3);
    }
    // The stretched piece's baseline sits at its slot bottom (21.6px from
    // the leaf top); 'x' keeps the row baseline, with its italic correction
    // kerning into the non-slanted closing fence.
    close(result.glyphs[0].origin.x(), 0.0);
    close(result.glyphs[0].origin.y(), 21.6);
    close(result.glyphs[1].origin.x(), 10.8);
    close(result.glyphs[1].origin.y(), 12.6);
    close(result.glyphs[2].origin.x(), 22.05);
    close(result.glyphs[2].origin.y(), 21.6);
    assert_eq!(snapshot(&result), include_str!("snapshots/fence-row.txt"));
    let mut fonts = provider(&stretch_font(700));
    assert_eq!(result, layout(&formula, &mut fonts, options(0)).unwrap());
}

fn styled(content: Formula, hints: logifos_model::PresentationStyle) -> Formula {
    Formula::Styled {
        style: hints,
        content: Box::new(content),
    }
}

#[test]
fn explicit_depth_inherits_adds_subtracts_and_resets() {
    use logifos_model::{PresentationStyle, ScriptLevel};
    let hint = |level| PresentationStyle {
        script_level: Some(level),
        ..Default::default()
    };
    let formula = styled(
        sequence(vec![
            symbol("x"),
            styled(sequence(vec![symbol("y")]), hint(ScriptLevel::Add(1))),
            styled(sequence(vec![symbol("z")]), hint(ScriptLevel::Absolute(0))),
            styled(sequence(vec![symbol("x")]), hint(ScriptLevel::Subtract(2))),
        ]),
        hint(ScriptLevel::Absolute(1)),
    );
    let result = render(&formula, 0);
    let sizes: Vec<_> = result.glyphs.iter().map(|g| g.font_size_px).collect();
    for (actual, expected) in sizes.into_iter().zip([14.4, 10.8, 18.0, 18.0 / 0.71]) {
        close(actual, expected);
    }
}

#[test]
fn root_display_and_nested_displaystyle_control_fraction_children() {
    use logifos_model::{MathDisplay, PresentationStyle};
    let frac = || Formula::Fraction(Box::new(symbol("x")), Box::new(symbol("y")));
    let formula = styled(
        sequence(vec![
            frac(),
            styled(
                Formula::Style(vec![frac()]),
                PresentationStyle {
                    displaystyle: Some(false),
                    ..Default::default()
                },
            ),
        ]),
        PresentationStyle {
            display: Some(MathDisplay::Block),
            ..Default::default()
        },
    );
    let result = render(&formula, 0);
    for (glyph, expected) in result.glyphs.iter().zip([18.0, 18.0, 14.4, 14.4]) {
        close(glyph.font_size_px, expected);
    }
    let overridden = styled(
        sequence(vec![frac()]),
        PresentationStyle {
            display: Some(MathDisplay::Block),
            displaystyle: Some(false),
            ..Default::default()
        },
    );
    for glyph in render(&overridden, 0).glyphs {
        close(glyph.font_size_px, 14.4);
    }
}

#[test]
fn effective_depth_outside_bounds_refuses_without_panicking() {
    use logifos_model::{PresentationStyle, ScriptLevel};
    let hint = |n| PresentationStyle {
        script_level: Some(ScriptLevel::Add(n)),
        ..Default::default()
    };
    let formula = styled(sequence(vec![styled(sequence(vec![]), hint(1))]), hint(255));
    assert_eq!(
        layout(&formula, &mut provider(&font()), options(0)).unwrap_err(),
        LayoutError::InvalidOptions
    );
}

#[test]
fn preserved_attributes_without_layout_semantics_refuse_instead_of_disappearing() {
    use logifos_model::{MathColor, MathSpace, MathVariant, PresentationStyle};
    for (hints, content) in [
        (
            PresentationStyle {
                math_variant: Some(MathVariant::Bold),
                ..Default::default()
            },
            Formula::Style(vec![symbol("x")]),
        ),
        (
            PresentationStyle {
                math_color: Some(MathColor::Red),
                ..Default::default()
            },
            Formula::Style(vec![symbol("x")]),
        ),
        (
            PresentationStyle {
                right_space: Some(MathSpace::Thick),
                ..Default::default()
            },
            operator("+"),
        ),
        (
            PresentationStyle {
                accent: Some(true),
                ..Default::default()
            },
            Formula::Over(Box::new(symbol("x")), Box::new(symbol("y"))),
        ),
    ] {
        let formula = sequence(vec![styled(content, hints)]);
        assert_eq!(
            layout(&formula, &mut provider(&font()), options(0)).unwrap_err(),
            LayoutError::NotLaidOutYet
        );
    }
}

#[test]
fn explicit_center_is_preserved_without_changing_default_table_geometry() {
    use logifos_model::{ColumnAlign, PresentationStyle};
    let hints = PresentationStyle {
        column_align: Some(ColumnAlign::Center),
        ..Default::default()
    };
    let plain = Formula::Table(vec![
        vec![sequence(vec![symbol("x")])],
        vec![sequence(vec![symbol("y"), symbol("z")])],
    ]);
    let explicit = Formula::Table(vec![
        vec![styled(sequence(vec![symbol("x")]), hints)],
        vec![sequence(vec![symbol("y"), symbol("z")])],
    ]);
    assert_eq!(render(&plain, 0), render(&explicit, 0));
    let invalid = sequence(vec![styled(sequence(vec![symbol("x")]), hints)]);
    assert_eq!(
        layout(&invalid, &mut provider(&font()), options(0)).unwrap_err(),
        LayoutError::InvalidOptions
    );
}
