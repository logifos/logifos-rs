use logifos_layout::{Bounds, GeometryError, Metrics, Point};

#[test]
fn metrics_match_top_left_css_pixel_contract() {
    let metrics = Metrics::new(12.5, 7.0, 2.25).unwrap();
    assert_eq!(metrics.width(), 12.5);
    assert_eq!(metrics.height(), 7.0);
    assert_eq!(metrics.baseline(), 7.0);
    assert_eq!(metrics.depth(), 2.25);
    assert_eq!(metrics.total_height(), 9.25);
}

#[test]
fn translations_compose_and_invert_for_exact_dyadic_coordinates() {
    for x in -4..=4 {
        for y in -4..=4 {
            let point = Point::new(f64::from(x) / 2.0, f64::from(y) / 4.0).unwrap();
            let a = Point::new(2.5, -1.25).unwrap();
            let b = Point::new(-0.5, 3.0).unwrap();
            assert_eq!(
                point.translated(a).unwrap().translated(b).unwrap(),
                point.translated(a.translated(b).unwrap()).unwrap()
            );
            assert_eq!(
                point
                    .translated(a)
                    .unwrap()
                    .translated(Point::new(-a.x(), -a.y()).unwrap())
                    .unwrap(),
                point
            );
        }
    }
}

#[test]
fn bounds_union_obeys_rectangle_algebra_on_exact_coordinates() {
    let a = Bounds::new(-2.0, 1.0, 3.0, 5.0).unwrap();
    let b = Bounds::new(0.0, -3.0, 4.0, 2.0).unwrap();
    let c = Bounds::new(1.0, 2.0, 2.0, 2.0).unwrap();
    assert_eq!(a.union(a).unwrap(), a);
    assert_eq!(a.union(b).unwrap(), b.union(a).unwrap());
    assert_eq!(
        a.union(b).unwrap().union(c).unwrap(),
        a.union(b.union(c).unwrap()).unwrap()
    );
    assert_eq!(
        a.union(b).unwrap(),
        Bounds::new(-2.0, -3.0, 6.0, 9.0).unwrap()
    );
    let offset = Point::new(1.5, -2.0).unwrap();
    assert_eq!(
        a.union(b).unwrap().translated(offset).unwrap(),
        a.translated(offset)
            .unwrap()
            .union(b.translated(offset).unwrap())
            .unwrap()
    );
}

#[test]
fn degenerate_bounds_are_retained_and_negative_origins_are_valid() {
    let point = Bounds::new(-3.0, -2.0, 0.0, 0.0).unwrap();
    assert_eq!(
        point
            .union(Bounds::new(1.0, 2.0, 2.0, 3.0).unwrap())
            .unwrap(),
        Bounds::new(-3.0, -2.0, 6.0, 7.0).unwrap()
    );
}

#[test]
fn constructors_reject_every_nonfinite_coordinate_or_dimension() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for result in [Point::new(value, 0.0), Point::new(0.0, value)] {
            assert_eq!(result, Err(GeometryError::NonFinite));
        }
        for result in [
            Metrics::new(value, 0.0, 0.0),
            Metrics::new(0.0, value, 0.0),
            Metrics::new(0.0, 0.0, value),
        ] {
            assert_eq!(result, Err(GeometryError::NonFinite));
        }
        for result in [
            Bounds::new(value, 0.0, 0.0, 0.0),
            Bounds::new(0.0, value, 0.0, 0.0),
            Bounds::new(0.0, 0.0, value, 0.0),
            Bounds::new(0.0, 0.0, 0.0, value),
        ] {
            assert_eq!(result, Err(GeometryError::NonFinite));
        }
    }
}

#[test]
fn negative_extents_are_refused() {
    for result in [
        Metrics::new(-1.0, 0.0, 0.0),
        Metrics::new(0.0, -1.0, 0.0),
        Metrics::new(0.0, 0.0, -1.0),
    ] {
        assert_eq!(result, Err(GeometryError::NegativeExtent));
    }
    for result in [
        Bounds::new(0.0, 0.0, -1.0, 0.0),
        Bounds::new(0.0, 0.0, 0.0, -1.0),
    ] {
        assert_eq!(result, Err(GeometryError::NegativeExtent));
    }
}

#[test]
fn all_stored_zeros_are_positive() {
    let point = Point::new(-0.0, -0.0).unwrap();
    let metrics = Metrics::new(-0.0, -0.0, -0.0).unwrap();
    let bounds = Bounds::new(-0.0, -0.0, -0.0, -0.0).unwrap();
    for value in [
        point.x(),
        point.y(),
        metrics.width(),
        metrics.height(),
        metrics.depth(),
        metrics.total_height(),
        bounds.origin().x(),
        bounds.origin().y(),
        bounds.width(),
        bounds.height(),
        Point::new(-1.0, 0.0)
            .unwrap()
            .translated(Point::new(1.0, 0.0).unwrap())
            .unwrap()
            .x(),
    ] {
        assert_eq!(value.to_bits(), 0.0_f64.to_bits());
    }
}

#[test]
fn finite_range_and_subnormal_inputs_are_preserved() {
    let tiny = f64::from_bits(1);
    assert_eq!(Metrics::new(tiny, tiny, 0.0).unwrap().height().to_bits(), 1);
    assert_eq!(Point::new(-f64::MAX, f64::MAX).unwrap().y(), f64::MAX);
    assert_eq!(
        Bounds::new(0.0, 0.0, f64::MAX, f64::MAX).unwrap().width(),
        f64::MAX
    );
}

#[test]
fn arithmetic_overflow_is_a_typed_error() {
    assert_eq!(
        Metrics::new(0.0, f64::MAX, f64::MAX),
        Err(GeometryError::Overflow)
    );
    for result in [
        Bounds::new(f64::MAX, 0.0, f64::MAX, 0.0),
        Bounds::new(0.0, f64::MAX, 0.0, f64::MAX),
    ] {
        assert_eq!(result, Err(GeometryError::Overflow));
    }
    for sign in [-1.0, 1.0] {
        let point = Point::new(sign * f64::MAX, sign * f64::MAX).unwrap();
        assert_eq!(point.translated(point), Err(GeometryError::Overflow));
    }
    let left = Bounds::new(-f64::MAX, 0.0, 0.0, 0.0).unwrap();
    let right = Bounds::new(f64::MAX, 0.0, 0.0, 0.0).unwrap();
    assert_eq!(left.union(right), Err(GeometryError::Overflow));
    let top = Bounds::new(0.0, -f64::MAX, 0.0, 0.0).unwrap();
    let bottom = Bounds::new(0.0, f64::MAX, 0.0, 0.0).unwrap();
    assert_eq!(top.union(bottom), Err(GeometryError::Overflow));
    let bounds = Bounds::new(0.0, 0.0, f64::MAX, 1.0).unwrap();
    assert_eq!(
        bounds.translated(Point::new(f64::MAX, 0.0).unwrap()),
        Err(GeometryError::Overflow)
    );
}
