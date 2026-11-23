use std::fmt::Write;

use logifos_layout::{Bounds, BoxKind, BoxTree, GeometryError, Kern, Metrics, StackItem};

fn rule(width: f64, height: f64, depth: f64) -> BoxTree {
    BoxTree::rule(width, height, depth).unwrap()
}

fn kern(amount: f64) -> StackItem {
    Kern::new(amount).unwrap().into()
}

fn snapshot(tree: &BoxTree) -> String {
    fn visit(tree: &BoxTree, x: f64, y: f64, level: usize, output: &mut String) {
        let m = tree.metrics();
        write!(
            output,
            "{}{:?}@({x},{y}) size=({},{},{}) baseline={} ink=",
            "  ".repeat(level),
            tree.kind(),
            m.width(),
            m.height(),
            m.depth(),
            m.baseline()
        )
        .unwrap();
        match tree.ink_bounds() {
            Some(b) => writeln!(
                output,
                "({},{},{},{})",
                b.origin().x(),
                b.origin().y(),
                b.width(),
                b.height()
            )
            .unwrap(),
            None => writeln!(output, "none").unwrap(),
        }
        for child in tree.children() {
            visit(
                child.node(),
                child.offset().x(),
                child.offset().y(),
                level + 1,
                output,
            );
        }
    }
    let mut output = String::new();
    visit(tree, 0.0, 0.0, 0, &mut output);
    output
}

#[test]
fn horizontal_snapshot_aligns_baselines_and_preserves_overhangs() {
    // Advances: 6 + 2 - 1 + 4 = 11. Baseline=max(4,6)=6, depth=max(1,2)=2.
    // First ink translates from (-1,-1,8,7) by (0,2); second starts at x=7.
    let first = BoxTree::leaf(
        Metrics::new(6.0, 4.0, 1.0).unwrap(),
        Some(Bounds::new(-1.0, -1.0, 8.0, 7.0).unwrap()),
    );
    let tree = BoxTree::hbox(vec![
        first.into(),
        kern(2.0),
        kern(-1.0),
        rule(4.0, 6.0, 2.0).into(),
    ])
    .unwrap();
    assert_eq!(
        snapshot(&tree),
        concat!(
            "HBox@(0,0) size=(11,6,2) baseline=6 ink=(-1,0,12,8)\n",
            "  Leaf@(0,2) size=(6,4,1) baseline=4 ink=(-1,-1,8,7)\n",
            "  Rule@(7,0) size=(4,6,2) baseline=6 ink=(0,0,4,8)\n",
        )
    );
    for child in tree.children() {
        assert_eq!(
            child.offset().y() + child.node().metrics().baseline(),
            tree.metrics().baseline()
        );
    }
}

#[test]
fn vertical_snapshot_has_explicit_baseline_and_signed_gaps() {
    // 5 + 2 + 7 - 1 = 13 total; baseline 8 leaves depth 5. Width=max(3,5)=5.
    let tree = BoxTree::vbox(
        vec![
            rule(3.0, 4.0, 1.0).into(),
            kern(2.0),
            rule(5.0, 6.0, 1.0).into(),
            kern(-1.0),
        ],
        8.0,
    )
    .unwrap();
    assert_eq!(
        snapshot(&tree),
        concat!(
            "VBox@(0,0) size=(5,8,5) baseline=8 ink=(0,0,5,14)\n",
            "  Rule@(0,0) size=(3,4,1) baseline=4 ink=(0,0,3,5)\n",
            "  Rule@(0,7) size=(5,6,1) baseline=6 ink=(0,0,5,7)\n",
        )
    );
}

#[test]
fn nested_snapshot_composes_local_coordinates() {
    let row = BoxTree::hbox(vec![
        rule(2.0, 3.0, 1.0).into(),
        kern(0.5),
        rule(1.0, 2.0, 0.0).into(),
    ])
    .unwrap();
    let tree = BoxTree::vbox(
        vec![kern(1.0), row.into(), kern(2.0), rule(4.0, 0.5, 0.0).into()],
        4.0,
    )
    .unwrap();
    assert_eq!(
        snapshot(&tree),
        concat!(
            "VBox@(0,0) size=(4,4,3.5) baseline=4 ink=(0,1,4,6.5)\n",
            "  HBox@(0,1) size=(3.5,3,1) baseline=3 ink=(0,0,3.5,4)\n",
            "    Rule@(0,0) size=(2,3,1) baseline=3 ink=(0,0,2,4)\n",
            "    Rule@(2.5,1) size=(1,2,0) baseline=2 ink=(0,0,1,2)\n",
            "  Rule@(0,7) size=(4,0.5,0) baseline=0.5 ink=(0,0,4,0.5)\n",
        )
    );
}

#[test]
fn empty_and_kern_only_stacks_have_no_ink() {
    for tree in [
        BoxTree::hbox(vec![]).unwrap(),
        BoxTree::vbox(vec![], -0.0).unwrap(),
    ] {
        assert_eq!(tree.metrics(), Metrics::default());
        assert_eq!(tree.ink_bounds(), None);
        assert!(tree.children().is_empty());
    }
    let horizontal = BoxTree::hbox(vec![kern(3.0), kern(-1.0)]).unwrap();
    assert_eq!(horizontal.metrics(), Metrics::new(2.0, 0.0, 0.0).unwrap());
    assert_eq!(horizontal.ink_bounds(), None);
    let vertical = BoxTree::vbox(vec![kern(3.0), kern(-1.0)], 1.5).unwrap();
    assert_eq!(vertical.metrics(), Metrics::new(0.0, 1.5, 0.5).unwrap());
    assert_eq!(vertical.ink_bounds(), None);
}

#[test]
fn spaces_do_not_add_origin_to_ink_union() {
    let space = BoxTree::space(Metrics::new(5.0, 7.0, 2.0).unwrap());
    let tree = BoxTree::hbox(vec![space.into(), rule(2.0, 1.0, 0.0).into()]).unwrap();
    assert_eq!(tree.metrics(), Metrics::new(7.0, 7.0, 2.0).unwrap());
    assert_eq!(
        tree.ink_bounds(),
        Some(Bounds::new(5.0, 6.0, 2.0, 1.0).unwrap())
    );
}

#[test]
fn rules_cover_total_height_and_zero_area_rules_are_inkless() {
    let tree = rule(5.0, 0.5, 0.25);
    assert_eq!(tree.kind(), BoxKind::Rule);
    assert_eq!(
        tree.ink_bounds(),
        Some(Bounds::new(0.0, 0.0, 5.0, 0.75).unwrap())
    );
    for tree in [rule(0.0, 3.0, 1.0), rule(2.0, 0.0, 0.0)] {
        assert_eq!(tree.ink_bounds(), None);
    }
}

#[test]
fn negative_kerns_allow_overlap_and_ink_outside_the_advance_area() {
    let row = BoxTree::hbox(vec![kern(-2.0), rule(5.0, 2.0, 1.0).into(), kern(-3.0)]).unwrap();
    assert_eq!(row.metrics().width(), 0.0);
    assert_eq!(
        row.ink_bounds(),
        Some(Bounds::new(-2.0, 0.0, 5.0, 3.0).unwrap())
    );
    let column = BoxTree::vbox(
        vec![kern(-2.0), rule(5.0, 2.0, 1.0).into(), kern(-1.0)],
        0.0,
    )
    .unwrap();
    assert_eq!(column.metrics().total_height(), 0.0);
    assert_eq!(
        column.ink_bounds(),
        Some(Bounds::new(0.0, -2.0, 5.0, 3.0).unwrap())
    );
}

#[test]
fn wrapping_a_box_preserves_metrics_and_ink() {
    let leaf = BoxTree::leaf(
        Metrics::new(4.0, 3.0, 1.0).unwrap(),
        Some(Bounds::new(-1.0, -2.0, 6.0, 8.0).unwrap()),
    );
    for tree in [
        BoxTree::hbox(vec![leaf.clone().into()]).unwrap(),
        BoxTree::vbox(vec![leaf.clone().into()], 3.0).unwrap(),
    ] {
        assert_eq!(tree.metrics(), leaf.metrics());
        assert_eq!(tree.ink_bounds(), leaf.ink_bounds());
    }
}

#[test]
fn vertical_baseline_can_be_at_either_edge() {
    let top = BoxTree::vbox(vec![rule(1.0, 2.0, 1.0).into()], 0.0).unwrap();
    let bottom = BoxTree::vbox(vec![rule(1.0, 2.0, 1.0).into()], 3.0).unwrap();
    assert_eq!(top.metrics(), Metrics::new(1.0, 0.0, 3.0).unwrap());
    assert_eq!(bottom.metrics(), Metrics::new(1.0, 3.0, 0.0).unwrap());
    assert_eq!(top.ink_bounds(), bottom.ink_bounds());
}

#[test]
fn ordered_binary64_operations_are_repeatable_and_not_reassociated() {
    let make = || BoxTree::hbox(vec![kern(1e16), kern(1.0), kern(-1e16)]).unwrap();
    let expected = make();
    for _ in 0..8 {
        assert_eq!(make(), expected);
        assert_eq!(snapshot(&make()), snapshot(&expected));
    }
    assert_eq!(expected.metrics().width().to_bits(), 0.0_f64.to_bits());
    assert_eq!(
        Kern::new(-0.0).unwrap().amount().to_bits(),
        0.0_f64.to_bits()
    );
    let reordered = BoxTree::hbox(vec![kern(1e16), kern(-1e16), kern(1.0)]).unwrap();
    assert_eq!(reordered.metrics().width(), 1.0);
}

#[test]
fn invalid_stack_dimensions_and_baselines_are_refused() {
    assert_eq!(
        BoxTree::hbox(vec![kern(-1.0)]),
        Err(GeometryError::NegativeExtent)
    );
    assert_eq!(
        BoxTree::vbox(vec![kern(-1.0)], 0.0),
        Err(GeometryError::NegativeExtent)
    );
    assert_eq!(
        BoxTree::vbox(vec![], -1.0),
        Err(GeometryError::NegativeExtent)
    );
    assert_eq!(
        BoxTree::vbox(vec![kern(2.0)], 3.0),
        Err(GeometryError::BaselineOutOfBounds)
    );
    assert_eq!(
        BoxTree::rule(-1.0, 1.0, 0.0),
        Err(GeometryError::NegativeExtent)
    );
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(Kern::new(value), Err(GeometryError::NonFinite));
        assert_eq!(BoxTree::vbox(vec![], value), Err(GeometryError::NonFinite));
        assert_eq!(
            BoxTree::rule(value, 1.0, 0.0),
            Err(GeometryError::NonFinite)
        );
    }
}

#[test]
fn stack_overflows_are_refused_including_intermediate_cursors() {
    for sign in [-1.0, 1.0] {
        assert_eq!(
            BoxTree::hbox(vec![
                kern(sign * f64::MAX),
                kern(sign * f64::MAX),
                kern(-sign * f64::MAX)
            ]),
            Err(GeometryError::Overflow)
        );
        assert_eq!(
            BoxTree::vbox(vec![kern(sign * f64::MAX), kern(sign * f64::MAX)], 0.0),
            Err(GeometryError::Overflow)
        );
    }
    let wide = BoxTree::space(Metrics::new(f64::MAX, 0.0, 0.0).unwrap());
    assert_eq!(
        BoxTree::hbox(vec![wide.clone().into(), wide.into()]),
        Err(GeometryError::Overflow)
    );
    let tall = BoxTree::space(Metrics::new(0.0, f64::MAX, 0.0).unwrap());
    let deep = BoxTree::space(Metrics::new(0.0, 0.0, f64::MAX).unwrap());
    assert_eq!(
        BoxTree::hbox(vec![tall.clone().into(), deep.clone().into()]),
        Err(GeometryError::Overflow)
    );
    assert_eq!(
        BoxTree::vbox(vec![tall.into(), deep.into()], 0.0),
        Err(GeometryError::Overflow)
    );
}

#[test]
fn translated_ink_and_union_overflows_are_refused() {
    let ink = BoxTree::leaf(
        Metrics::default(),
        Some(Bounds::new(f64::MAX, 0.0, 0.0, 0.0).unwrap()),
    );
    assert_eq!(
        BoxTree::hbox(vec![kern(f64::MAX), ink.into()]),
        Err(GeometryError::Overflow)
    );
    let top_ink = BoxTree::leaf(
        Metrics::default(),
        Some(Bounds::new(0.0, f64::MAX, 0.0, 0.0).unwrap()),
    );
    assert_eq!(
        BoxTree::vbox(vec![kern(f64::MAX), top_ink.into()], 0.0),
        Err(GeometryError::Overflow)
    );
    let left = BoxTree::leaf(
        Metrics::default(),
        Some(Bounds::new(-f64::MAX, 0.0, 0.0, 0.0).unwrap()),
    );
    let right = BoxTree::leaf(
        Metrics::default(),
        Some(Bounds::new(f64::MAX, 0.0, 0.0, 0.0).unwrap()),
    );
    assert_eq!(
        BoxTree::hbox(vec![left.into(), right.into()]),
        Err(GeometryError::Overflow)
    );
}
