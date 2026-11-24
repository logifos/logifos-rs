mod support;
use logifos_layout::font::{
    math::{Direction, MathConstant, MathTable},
    FontErrorKind, FontFace, FontProvider, FontResource, StretchShape, MAX_FONT_BYTES,
};
use support::*;

fn resource<'a>(id: &'a str, bytes: &'a [u8], hash: &'a str, face_index: u16) -> FontResource<'a> {
    FontResource {
        id,
        bytes,
        sha256: hash,
        face_index,
    }
}

#[test]
fn fontisan_metrics_and_css_pixel_scaling() {
    let face = FontFace::load(&font(), 0).unwrap();
    assert_eq!(
        (
            face.metrics.units_per_em,
            face.metrics.ascent,
            face.metrics.descent,
            face.metrics.line_gap
        ),
        (1000, 800, -200, 50)
    );
    assert_eq!(face.glyph_id('x'), Some(1));
    assert_eq!(face.glyph_id('𝑥'), Some(4));
    assert_eq!(face.glyph_id('z'), None);
    assert_eq!(
        (
            face.glyph_metrics(1).unwrap().advance,
            face.glyph_metrics(1).unwrap().left_side_bearing
        ),
        (600, -10)
    );
    assert_eq!(
        (
            face.glyph_metrics(5).unwrap().advance,
            face.glyph_metrics(5).unwrap().left_side_bearing
        ),
        (600, 20)
    );
    assert_eq!(face.scale(600.0, 20.0).unwrap(), 12.0);
    assert_eq!(face.scale(-0.0, 16.0).unwrap().to_bits(), 0);
    assert_eq!(
        face.glyph_metrics(6).unwrap_err().kind,
        FontErrorKind::MissingGlyph
    );
}

#[test]
fn math_constants_have_specification_order_and_signed_values() {
    let math = MathTable::parse(&math_table(250, true), 6).unwrap();
    let c = math.constants;
    assert_eq!(
        (
            c.script_percent_scale_down,
            c.script_script_percent_scale_down,
            c.radical_degree_bottom_raise_percent
        ),
        (80, 60, 65)
    );
    assert_eq!(
        (
            c.delimited_sub_formula_min_height,
            c.display_operator_min_height
        ),
        (900, 1100)
    );
    for (field, value) in [
        (MathConstant::MathLeading, 100),
        (MathConstant::AxisHeight, 250),
        (MathConstant::FractionRuleThickness, 134),
        (MathConstant::RadicalRuleThickness, 147),
        (MathConstant::RadicalKernAfterDegree, -120),
    ] {
        assert_eq!(c.value(field), value);
    }
}

#[test]
fn coverage_formats_italics_and_top_accent_defaults() {
    let b = font();
    let hash = digest(&b);
    let mut p = FontProvider::new(&[resource("math", &b, &hash, 0)], &["math"]).unwrap();
    let x = p.resolve('x', 20.0).unwrap();
    assert_eq!(
        (x.advance_px, x.italics_correction_px, x.top_accent_px),
        (12.0, 0.5, 6.8)
    );
    let paren = p.resolve('(', 20.0).unwrap();
    assert_eq!(
        (paren.italics_correction_px, paren.top_accent_px),
        (0.0, 6.0)
    );
}

#[test]
fn variants_and_assembly_keep_direction_and_part_order() {
    let math = MathTable::parse(&math_table(250, true), 6).unwrap();
    assert_eq!(math.min_connector_overlap, 40);
    let v = math.construction(2, Direction::Vertical).unwrap();
    assert_eq!(
        v.variants
            .iter()
            .map(|v| (v.glyph_id, v.advance))
            .collect::<Vec<_>>(),
        [(2, 800), (3, 1200)]
    );
    let a = v.assembly.as_ref().unwrap();
    assert_eq!(a.italics_correction, 35);
    assert_eq!(
        a.parts
            .iter()
            .map(|p| (p.glyph_id, p.extender))
            .collect::<Vec<_>>(),
        [(3, false), (4, true), (5, false)]
    );
    assert_eq!(
        (
            a.parts[1].start_connector,
            a.parts[1].end_connector,
            a.parts[1].full_advance
        ),
        (60, 60, 600)
    );
    let h = math.construction(1, Direction::Horizontal).unwrap();
    assert_eq!((h.variants[0].glyph_id, h.variants[0].advance), (5, 1600));
    assert!(math.construction(2, Direction::Horizontal).is_none());
}

#[test]
fn fallback_preserves_primary_constants_and_first_use_identity() {
    let a = FontBuilder::new(&[('x', 1)], 250, false).finish();
    let ah = digest(&a);
    let b = FontBuilder::new(&[('y', 1)], 400, true).finish();
    let bh = digest(&b);
    let mut p = FontProvider::new(
        &[resource("b", &b, &bh, 0), resource("a", &a, &ah, 0)],
        &["a", "b"],
    )
    .unwrap();
    assert!(p.fonts_used().is_empty());
    assert_eq!(
        p.math_constant_px(MathConstant::AxisHeight, 20.0).unwrap(),
        5.0
    );
    assert_eq!(p.resolve('y', 20.0).unwrap().resource_id, "b");
    assert_eq!(p.resolve('x', 20.0).unwrap().resource_id, "a");
    p.resolve('y', 20.0).unwrap();
    assert_eq!(
        p.fonts_used(),
        [
            p.resolve('x', 20.0).unwrap().identity,
            p.resolve('y', 20.0).unwrap().identity
        ]
    );
    let used = p.fonts_used();
    assert_eq!(p.resolve('z', 20.0).unwrap_err().code(), "MISSING_GLYPH");
    assert_eq!(p.fonts_used(), used);
    let mut fresh = FontProvider::new(&[resource("b", &b, &bh, 0)], &["b"]).unwrap();
    assert!(fresh.fonts_used().is_empty());
    assert_eq!(
        fresh
            .math_constant_px(MathConstant::AxisHeight, 20.0)
            .unwrap(),
        8.0
    );
}

#[test]
fn first_covering_font_wins_and_aliases_deduplicate_by_file_face() {
    let b = font();
    let hash = digest(&b);
    let mut p = FontProvider::new(
        &[resource("a", &b, &hash, 0), resource("alias", &b, &hash, 0)],
        &["alias", "a"],
    )
    .unwrap();
    assert_eq!(p.resolve('x', 16.0).unwrap().resource_id, "alias");
    p.resolve('(', 16.0).unwrap();
    assert_eq!(p.fonts_used().len(), 1);
}

#[test]
fn stretch_fallback_uses_first_font_with_requested_construction() {
    let a = FontBuilder::new(&[('(', 2)], 250, false).finish();
    let ah = digest(&a);
    let b = font();
    let bh = digest(&b);
    let mut p = FontProvider::new(
        &[resource("a", &a, &ah, 0), resource("b", &b, &bh, 0)],
        &["a", "b"],
    )
    .unwrap();
    let v = p
        .resolve_stretch('(', Direction::Vertical, 20.0, 20.0)
        .unwrap();
    assert_eq!(v.resource_id, "b");
    assert!(matches!(v.shape, StretchShape::Variant(v) if v.glyph_id == 3));
    let a = p
        .resolve_stretch('(', Direction::Vertical, 100.0, 20.0)
        .unwrap();
    assert!(matches!(a.shape, StretchShape::Assembly(_)));
    assert_eq!(a.min_connector_overlap, 40);
    assert_eq!(p.fonts_used().len(), 1);
    assert_eq!(
        p.resolve_stretch('(', Direction::Horizontal, 10.0, 20.0)
            .unwrap_err()
            .code(),
        "MISSING_GLYPH"
    );
}

#[test]
fn collection_faces_have_distinct_identity_and_metrics() {
    let b = collection();
    let hash = digest(&b);
    let mut p = FontProvider::new(
        &[
            resource("zero", &b, &hash, 0),
            resource("one", &b, &hash, 1),
        ],
        &["zero", "one"],
    )
    .unwrap();
    assert_eq!(p.resolve('x', 16.0).unwrap().identity.face_index, 0);
    assert_eq!(p.resolve('y', 16.0).unwrap().identity.face_index, 1);
    assert_eq!(p.fonts_used().len(), 2);
    assert_eq!(p.fonts_used()[0].sha256, p.fonts_used()[1].sha256);
    assert!(FontFace::load(&b, 2).is_err());
    assert!(FontFace::load(&font(), 1).is_err());
}

#[test]
fn resource_identity_order_and_limits_refuse_typed() {
    let b = font();
    let hash = digest(&b);
    let r = resource("valid", &b, &hash, 0);
    for order in [vec![], vec!["other"], vec!["valid", "valid"]] {
        assert!(FontProvider::new(&[r], &order).is_err());
    }
    assert_eq!(
        FontProvider::new(&[r, r], &["valid", "valid"])
            .unwrap_err()
            .kind,
        FontErrorKind::InvalidResources
    );
    assert_eq!(
        FontProvider::new(&[resource("valid", &b, &"0".repeat(64), 0)], &["valid"])
            .unwrap_err()
            .code(),
        "FONT_HASH_MISMATCH"
    );
    for id in ["", "9bad", "with space", "é"] {
        assert_eq!(
            FontProvider::new(&[resource(id, &b, &hash, 0)], &[id])
                .unwrap_err()
                .kind,
            FontErrorKind::InvalidResources
        );
    }
    for hash in ["é".repeat(32), "A".repeat(64), "0".repeat(63)] {
        assert_eq!(
            FontProvider::new(&[resource("valid", &b, &hash, 0)], &["valid"])
                .unwrap_err()
                .kind,
            FontErrorKind::InvalidResources
        );
    }
    assert_eq!(
        FontProvider::new(&[], &[]).unwrap_err().kind,
        FontErrorKind::ResourceLimit
    );
    assert_eq!(
        FontFace::load(&vec![0; MAX_FONT_BYTES + 1], 0)
            .unwrap_err()
            .kind,
        FontErrorKind::ResourceLimit
    );
}

#[test]
fn validates_every_resource_even_when_unused() {
    let b = font();
    let hash = digest(&b);
    let mut broken = FontBuilder::new(&[('y', 1)], 250, true);
    broken.tables.remove(b"MATH");
    let bad = broken.finish();
    let bad_hash = digest(&bad);
    assert_eq!(
        FontProvider::new(
            &[
                resource("good", &b, &hash, 0),
                resource("bad", &bad, &bad_hash, 0)
            ],
            &["good", "bad"]
        )
        .unwrap_err()
        .kind,
        FontErrorKind::MissingTable
    );
}

#[test]
fn nonfinite_sizes_refuse_and_glyf_points_decode() {
    let b = font();
    let face = FontFace::load(&b, 0).unwrap();
    for size in [0.0, -1.0, f64::INFINITY, f64::NAN, 4097.0] {
        assert_eq!(
            face.scale(600.0, size).unwrap_err().kind,
            FontErrorKind::InvalidSize
        );
    }
    assert!(face.scale(f64::MAX, 4096.0).is_err());
    assert!(face.scale(f64::NAN, 16.0).is_err());
    let outline = face.outline(1).unwrap().unwrap();
    assert_eq!(outline.contours.len(), 1);
    assert_eq!(
        outline.contours[0]
            .points
            .iter()
            .map(|p| (p.x, p.y, p.on_curve))
            .collect::<Vec<_>>(),
        vec![(0, -200, true), (500, 700, true), (0, -200, true)]
    );
}

#[test]
fn every_truncated_math_prefix_is_a_typed_error() {
    let bytes = math_table(250, true);
    for len in 0..bytes.len() {
        assert!(MathTable::parse(&bytes[..len], 6).is_err(), "prefix {len}");
    }
}

#[test]
fn every_truncated_font_prefix_is_a_typed_error() {
    let bytes = font();
    // sfnt permits alignment padding to be absent at EOF.
    let required = (0..get16(&bytes, 4))
        .map(|i| {
            let at = 12 + i * 16;
            get32(&bytes, at + 8) + get32(&bytes, at + 12)
        })
        .max()
        .unwrap();
    for len in 0..required {
        assert!(FontFace::load(&bytes[..len], 0).is_err(), "prefix {len}");
    }
}

#[test]
fn malformed_math_offsets_counts_and_glyph_ids_refuse() {
    let bytes = math_table(250, true);
    let info = get16(&bytes, 6);
    let italics = info + get16(&bytes, info);
    let accent = info + get16(&bytes, info + 2);
    let variants = get16(&bytes, 8);
    let construction = variants + get16(&bytes, variants + 10);
    let assembly = construction + get16(&bytes, construction);
    let kern = info + get16(&bytes, info + 6);
    let kern_value = kern + get16(&bytes, kern + 4);
    let cases = [
        (0, 2),
        (4, 0),
        (4, 2),
        (6, 0),
        (8, 0),
        (4, u16::MAX),
        (info, u16::MAX),
        (info + 2, u16::MAX),
        (info + 4, u16::MAX),
        (info + 6, u16::MAX),
        (italics, 2),
        (italics + 2, u16::MAX),
        (italics + 10, 2),
        (italics + 12, 6),
        (accent + 8, 3),
        (accent + 16, 1),
        (variants + 2, 0),
        (variants + 4, u16::MAX),
        (variants + 6, u16::MAX),
        (variants + 8, u16::MAX),
        (variants + 10, u16::MAX),
        (construction, u16::MAX),
        (construction + 2, u16::MAX),
        (construction + 4, 6),
        (construction + 10, 100),
        (assembly + 2, u16::MAX),
        (assembly + 4, 0),
        (assembly + 4, u16::MAX),
        (assembly + 6, 6),
        (assembly + 8, 501),
        (assembly + 14, 2),
        (kern, 0),
        (kern + 2, u16::MAX),
        (kern + 4, u16::MAX),
        (kern_value, u16::MAX),
    ];
    for (at, value) in cases {
        let mut b = bytes.clone();
        put16(&mut b, at, value);
        assert!(MathTable::parse(&b, 6).is_err(), "field {at} = {value}");
    }
}

#[test]
fn device_offsets_use_each_immediate_parent_and_validate_payload() {
    let mut bytes = math_table(250, true);
    let constants = get16(&bytes, 4);
    let info = get16(&bytes, 6);
    let italics = info + get16(&bytes, info);
    let variants = get16(&bytes, 8);
    let construction = variants + get16(&bytes, variants + 10);
    let assembly = construction + get16(&bytes, construction);
    for (parent, field) in [
        (constants, constants + 10),
        (italics, italics + 6),
        (assembly, assembly + 2),
    ] {
        let device = bytes.len();
        put16(&mut bytes, field, (device - parent) as u16);
        bytes.extend(words(&[12, 16, 1, 0]));
        assert!(MathTable::parse(&bytes, 6).is_ok());
        let mut broken = bytes.clone();
        put16(&mut broken, device + 2, 11);
        assert!(MathTable::parse(&broken, 6).is_err());
        let mut broken = bytes.clone();
        put16(&mut broken, device + 4, 4);
        assert!(MathTable::parse(&broken, 6).is_err());
        let mut broken = bytes.clone();
        put16(&mut broken, device + 2, 100);
        assert!(MathTable::parse(&broken, 6).is_err());
    }
}

#[test]
fn malformed_directory_metrics_cmap_and_loca_refuse() {
    for (tag, at, value) in [
        (*b"head", 18, 0),
        (*b"hhea", 34, 0),
        (*b"hhea", 34, 7),
        (*b"maxp", 4, 0),
        (*b"cmap", 2, u16::MAX),
        (*b"cmap", 12, 11),
        (*b"loca", 2, u16::MAX),
    ] {
        let mut b = FontBuilder::new(&[('x', 1)], 250, true);
        put16(b.tables.get_mut(&tag).unwrap(), at, value);
        assert!(FontFace::load(&b.finish(), 0).is_err(), "{tag:?}");
    }
    for tag in [
        *b"head", *b"hhea", *b"hmtx", *b"cmap", *b"maxp", *b"MATH", *b"loca",
    ] {
        let mut b = FontBuilder::new(&[('x', 1)], 250, true);
        b.tables.insert(tag, vec![]);
        assert!(FontFace::load(&b.finish(), 0).is_err(), "empty {tag:?}");
    }
    let mut bytes = font();
    put32(&mut bytes, 20, u32::MAX);
    assert!(FontFace::load(&bytes, 0).is_err());
    let mut bytes = font();
    put32(&mut bytes, 24, u32::MAX);
    assert!(FontFace::load(&bytes, 0).is_err());
    let mut bytes = collection();
    put32(&mut bytes, 8, u32::MAX);
    assert!(FontFace::load(&bytes, 0).is_err());
    let mut bytes = collection();
    put32(&mut bytes, 12, u32::MAX);
    assert!(FontFace::load(&bytes, 0).is_err());
}

#[test]
fn cmap_format4_delta_and_range_offset_are_checked() {
    let mut b = FontBuilder::new(&[('x', 1)], 250, true);
    let mut cmap = words(&[0, 1, 3, 1]);
    cmap.extend(12u32.to_be_bytes());
    cmap.extend(words(&[
        4, 34, 0, 4, 4, 1, 0, 120, 0xffff, 0, 120, 0xffff, 0, 1, 4, 0, 1,
    ]));
    b.tables.insert(*b"cmap", cmap.clone());
    assert_eq!(
        FontFace::load(&b.finish(), 0).unwrap().glyph_id('x'),
        Some(1)
    );
    for (at, value) in [(18, 3), (26, 119), (40, 3), (40, u16::MAX), (44, 6)] {
        let mut invalid = cmap.clone();
        put16(&mut invalid, at, value);
        b.tables.insert(*b"cmap", invalid);
        assert!(FontFace::load(&b.finish(), 0).is_err(), "{at}");
    }
    put16(&mut cmap, 36, (1i16 - 120i16) as u16);
    put16(&mut cmap, 40, 0);
    b.tables.insert(*b"cmap", cmap);
    assert_eq!(
        FontFace::load(&b.finish(), 0).unwrap().glyph_id('x'),
        Some(1)
    );
}

#[test]
fn deterministic_mutations_never_panic() {
    let bytes = font();
    for i in 0..bytes.len() {
        let mut altered = bytes.clone();
        altered[i] ^= 0xff;
        let _ = FontFace::load(&altered, 0);
    }
}

#[test]
fn each_fallback_font_uses_its_own_units_per_em() {
    let a = font();
    let ah = digest(&a);
    let mut b = FontBuilder::new(&[('y', 1)], 400, true);
    put16(b.tables.get_mut(b"head").unwrap(), 18, 2000);
    let b = b.finish();
    let bh = digest(&b);
    let mut p = FontProvider::new(
        &[
            resource("primary", &a, &ah, 0),
            resource("fallback", &b, &bh, 0),
        ],
        &["primary", "fallback"],
    )
    .unwrap();
    assert_eq!(p.resolve('y', 20.0).unwrap().advance_px, 6.0);
    assert_eq!(
        p.math_constant_px(MathConstant::AxisHeight, 20.0).unwrap(),
        5.0
    );
    assert_eq!(
        p.fonts_used()[0].sha256,
        FontFace::load(&b, 0).unwrap().identity.sha256
    );
}

#[test]
fn invalid_lookup_options_do_not_mark_resources_used() {
    let b = font();
    let hash = digest(&b);
    let mut p = FontProvider::new(&[resource("font", &b, &hash, 0)], &["font"]).unwrap();
    assert!(p
        .math_constant_px(MathConstant::AxisHeight, f64::NAN)
        .is_err());
    assert!(p.resolve('x', 0.0).is_err());
    for minimum in [f64::NAN, f64::INFINITY, -1.0] {
        assert!(p
            .resolve_stretch('(', Direction::Vertical, minimum, 16.0)
            .is_err());
    }
    assert!(p.fonts_used().is_empty());
}

#[test]
fn resource_count_and_combined_byte_budgets_are_enforced() {
    let b = font();
    let hash = digest(&b);
    let ids: Vec<_> = (0..17).map(|i| format!("font{i}")).collect();
    let resources: Vec<_> = ids.iter().map(|id| resource(id, &b, &hash, 0)).collect();
    let order: Vec<_> = ids.iter().map(String::as_str).collect();
    assert_eq!(
        FontProvider::new(&resources, &order).unwrap_err().kind,
        FontErrorKind::ResourceLimit
    );
    let mut padded = b;
    padded.resize(MAX_FONT_BYTES / 2 + 1, 0);
    let hash = digest(&padded);
    assert_eq!(
        FontProvider::new(
            &[
                resource("a", &padded, &hash, 0),
                resource("b", &padded, &hash, 0)
            ],
            &["a", "b"]
        )
        .unwrap_err()
        .kind,
        FontErrorKind::ResourceLimit
    );
}

#[test]
fn empty_optional_math_tables_are_valid() {
    let mut bytes = math_table(250, false);
    let info = get16(&bytes, 6);
    for field in [0, 2, 4, 6] {
        put16(&mut bytes, info + field, 0);
    }
    let math = MathTable::parse(&bytes, 6).unwrap();
    assert_eq!(math.italics_correction(1), 0);
    assert_eq!(math.top_accent(1), None);
    assert!(math.construction(1, Direction::Vertical).is_none());
}

#[test]
fn aliased_math_offsets_cannot_expand_without_a_budget() {
    let count = 9000usize;
    let mut bytes = math_table(250, false);
    let at = get16(&bytes, 8);
    bytes.truncate(at);
    let mut variants = vec![0; 10 + count * 2];
    put16(&mut variants, 2, (10 + count * 2) as u16);
    put16(&mut variants, 6, count as u16);
    variants.extend(words(&[2, 1, 0, (count - 1) as u16, 0]));
    let construction = variants.len();
    for i in 0..count {
        put16(&mut variants, 10 + i * 2, construction as u16);
    }
    variants.extend(words(&[0, 64]));
    for _ in 0..64 {
        variants.extend(words(&[1, 100]));
    }
    bytes.extend(variants);
    assert_eq!(
        MathTable::parse(&bytes, (count + 1) as u16)
            .unwrap_err()
            .kind,
        FontErrorKind::ResourceLimit
    );
}

#[test]
fn duplicate_sfnt_tags_and_invalid_cmap_groups_refuse() {
    let mut bytes = font();
    let first = bytes[12..16].to_vec();
    bytes[28..32].copy_from_slice(&first);
    assert!(FontFace::load(&bytes, 0).is_err());
    for (field, value) in [
        (8, u32::MAX),
        (16, u32::MAX),
        (28, 0x11_0000),
        (32, u32::MAX),
        (36, 6),
    ] {
        let mut b = FontBuilder::new(&[('x', 1), ('y', 2)], 250, true);
        put32(b.tables.get_mut(b"cmap").unwrap(), field, value);
        assert!(
            FontFace::load(&b.finish(), 0).is_err(),
            "cmap field {field}"
        );
    }
}

#[test]
fn subnormal_font_size_does_not_turn_zero_stretch_into_an_assembly() {
    let bytes = font();
    let hash = digest(&bytes);
    let mut provider = FontProvider::new(&[resource("font", &bytes, &hash, 0)], &["font"]).unwrap();
    let result = provider
        .resolve_stretch('(', Direction::Vertical, 0.0, f64::from_bits(1))
        .unwrap();
    assert!(matches!(result.shape, StretchShape::Variant(v) if v.glyph_id == 2));
}
