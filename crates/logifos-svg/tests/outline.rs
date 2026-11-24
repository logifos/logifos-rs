use logifos_layout::font::{ContourPx, GlyphOutlinePx, PointPx};
use logifos_layout::{GeometryError, Point};
use logifos_svg::outline::{GlyphOutline, OutlineError, PathCommand};
use PathCommand::{Close, CubicTo, LineTo, MoveTo, QuadTo};

fn point(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

fn font_outline(contours: &[&[(f64, f64, bool)]]) -> GlyphOutlinePx {
    GlyphOutlinePx {
        contours: contours
            .iter()
            .map(|points| ContourPx {
                points: points
                    .iter()
                    .map(|&(x, y, on_curve)| PointPx { x, y, on_curve })
                    .collect(),
            })
            .collect(),
    }
}

#[test]
fn all_commands_have_exact_absolute_svg_bytes() {
    let commands = vec![
        MoveTo(point(0.0, 0.0)),
        LineTo(point(1.5, -2.0)),
        QuadTo {
            control: point(2.0, 3.0),
            to: point(4.0, 0.0),
        },
        CubicTo {
            control1: point(5.0, 6.0),
            control2: point(7.0, -8.0),
            to: point(9.0, 0.0),
        },
        Close,
        MoveTo(point(-1.0, 2.0)),
        LineTo(point(-3.0, 4.0)),
        Close,
    ];
    let outline = GlyphOutline::new(commands.clone()).unwrap();
    assert_eq!(outline.commands(), commands);
    assert_eq!(
        outline.to_svg_path(point(10.0, 20.0)).unwrap(),
        "M10 20 L11.5 22 Q12 17 14 20 C15 14 17 28 19 20 ZM9 18 L7 16 Z"
    );
    // Placement never mutates the source outline, including control points.
    assert_eq!(
        outline.to_svg_path(Point::default()).unwrap(),
        "M0 0 L1.5 2 Q2 -3 4 0 C5 -6 7 8 9 0 ZM-1 -2 L-3 -4 Z"
    );
    for _ in 0..8 {
        assert_eq!(
            outline.clone().to_svg_path(point(10.0, 20.0)).unwrap(),
            "M10 20 L11.5 22 Q12 17 14 20 C15 14 17 28 19 20 ZM9 18 L7 16 Z"
        );
    }
}

#[test]
fn empty_outlines_emit_empty_path_data() {
    for outline in [
        GlyphOutline::default(),
        GlyphOutline::new(vec![]).unwrap(),
        GlyphOutline::try_from(&font_outline(&[])).unwrap(),
        GlyphOutline::try_from(&font_outline(&[&[], &[(1.0, 2.0, true)]])).unwrap(),
    ] {
        assert!(outline.commands().is_empty());
        assert_eq!(outline.to_svg_path(point(3.0, 4.0)).unwrap(), "");
    }
}

#[test]
fn invalid_contour_sequences_are_refused() {
    let p = Point::default();
    for commands in [
        vec![LineTo(p), Close],
        vec![QuadTo { control: p, to: p }, Close],
        vec![
            CubicTo {
                control1: p,
                control2: p,
                to: p,
            },
            Close,
        ],
        vec![Close],
        vec![MoveTo(p)],
        vec![MoveTo(p), LineTo(p)],
        vec![MoveTo(p), MoveTo(p), Close],
        vec![MoveTo(p), Close, Close],
        vec![MoveTo(p), Close, LineTo(p)],
        vec![MoveTo(p), Close, MoveTo(p)],
    ] {
        let error = GlyphOutline::new(commands).unwrap_err();
        assert_eq!(error, OutlineError::InvalidCommandOrder);
        assert!(std::error::Error::source(&error).is_none());
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn font_contour_and_winding_order_are_preserved() {
    let source = font_outline(&[
        &[(0.0, 0.0, true), (4.0, 0.0, true), (4.0, 4.0, true)],
        &[(1.0, 1.0, true), (2.0, 2.0, true), (2.0, 1.0, true)],
    ]);
    let outline = GlyphOutline::try_from(&source).unwrap();
    assert_eq!(
        outline.to_svg_path(point(0.0, 4.0)).unwrap(),
        "M0 4 L4 4 L4 0 ZM1 3 L2 2 L2 3 Z"
    );
    assert_eq!(outline.commands().len(), 8);
}

#[test]
fn first_off_curve_point_wraps_to_the_first_on_curve_point() {
    let source = font_outline(&[&[(0.0, 2.0, false), (2.0, 2.0, true), (2.0, 0.0, true)]]);
    let outline = GlyphOutline::try_from(&source).unwrap();
    assert_eq!(
        outline.commands(),
        [
            MoveTo(point(2.0, 2.0)),
            LineTo(point(2.0, 0.0)),
            QuadTo {
                control: point(0.0, 2.0),
                to: point(2.0, 2.0),
            },
            Close,
        ]
    );
    assert_eq!(
        outline.to_svg_path(Point::default()).unwrap(),
        "M2 -2 L2 0 Q0 -2 2 -2 Z"
    );
}

#[test]
fn off_curve_runs_include_the_wrap_midpoint() {
    let source = font_outline(&[&[(0.0, 0.0, false), (2.0, 2.0, false), (4.0, 0.0, false)]]);
    let outline = GlyphOutline::try_from(&source).unwrap();
    assert_eq!(
        outline.to_svg_path(point(0.0, 4.0)).unwrap(),
        "M1 3 Q2 2 3 3 Q4 4 2 4 Q0 4 1 3 Z"
    );
    assert_eq!(outline.commands().len(), 5);
}

#[test]
fn consecutive_controls_insert_midpoints_without_changing_the_source() {
    let source = font_outline(&[&[
        (0.0, 0.0, true),
        (2.0, 4.0, false),
        (4.0, 4.0, false),
        (6.0, 0.0, true),
    ]]);
    let original = source.clone();
    let outline = GlyphOutline::try_from(&source).unwrap();
    assert_eq!(
        outline.to_svg_path(Point::default()).unwrap(),
        "M0 0 Q2 -4 3 -4 Q4 -4 6 0 Z"
    );
    assert_eq!(source, original);
}

#[test]
fn midpoint_of_large_finite_controls_does_not_overflow() {
    let source = font_outline(&[&[(f64::MAX, f64::MAX, false), (f64::MAX, f64::MAX, false)]]);
    let outline = GlyphOutline::try_from(&source).unwrap();
    assert_eq!(outline.commands()[0], MoveTo(point(f64::MAX, f64::MAX)));
    assert!(!outline
        .to_svg_path(Point::default())
        .unwrap()
        .contains("inf"));
}

#[test]
fn non_finite_font_points_are_refused_even_in_single_point_contours() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for (x, y) in [(value, 0.0), (0.0, value)] {
            let source = font_outline(&[&[(x, y, true)]]);
            let error = GlyphOutline::try_from(&source).unwrap_err();
            assert_eq!(error, OutlineError::Geometry(GeometryError::NonFinite));
            assert!(std::error::Error::source(&error).is_some());
            assert!(error.to_string().contains("finite"));
        }
    }
}

#[test]
fn placement_checks_endpoints_and_both_curve_controls_on_both_axes() {
    let p = Point::default();
    for huge in [point(f64::MAX, 0.0), point(0.0, -f64::MAX)] {
        for command in [
            MoveTo(huge),
            LineTo(huge),
            QuadTo {
                control: huge,
                to: p,
            },
            QuadTo {
                control: p,
                to: huge,
            },
            CubicTo {
                control1: huge,
                control2: p,
                to: p,
            },
            CubicTo {
                control1: p,
                control2: huge,
                to: p,
            },
            CubicTo {
                control1: p,
                control2: p,
                to: huge,
            },
        ] {
            let mut commands = vec![];
            if !matches!(command, MoveTo(_)) {
                commands.push(MoveTo(p));
            }
            commands.extend([command, Close]);
            let outline = GlyphOutline::new(commands).unwrap();
            assert_eq!(
                outline.to_svg_path(point(f64::MAX, f64::MAX)),
                Err(OutlineError::Geometry(GeometryError::Overflow))
            );
        }
    }
}

#[test]
fn path_numbers_have_exact_lossless_formatting_and_normalize_signed_zero() {
    let outline = GlyphOutline::new(vec![
        MoveTo(point(-0.0, 0.0)),
        LineTo(point(1.2345678901234567, -0.000000123456789)),
        Close,
    ])
    .unwrap();
    assert_eq!(
        outline.to_svg_path(point(-0.0, -0.0)).unwrap(),
        "M0 0 L1.2345678901234567 0.000000123456789 Z"
    );
    for value in [
        f64::from_bits(1),
        f64::MIN_POSITIVE,
        1.2345678901234567,
        1e20,
        f64::MAX,
    ] {
        let outline = GlyphOutline::new(vec![MoveTo(point(value, -value)), Close]).unwrap();
        let data = outline.to_svg_path(Point::default()).unwrap();
        let coordinates = data.strip_prefix('M').unwrap().strip_suffix(" Z").unwrap();
        for coordinate in coordinates.split(' ') {
            assert_eq!(
                coordinate.parse::<f64>().unwrap().to_bits(),
                value.to_bits()
            );
        }
    }
}
