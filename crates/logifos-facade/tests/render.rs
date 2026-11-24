mod support;

use base64::{engine::general_purpose::STANDARD, Engine};
use logifos_facade::request_json;
use logifos_model::FORMULA_CONTRACT;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const MATHML: &str = "<math><mi>x</mi><mo>+</mo><mn>1</mn></math>";

fn parse_model() -> Value {
    let request = json!({
        "abi_version": 1, "schema_version": 1,
        "op": "parse", "format": "mathml", "source": MATHML,
    });
    let reply = request_json(request.to_string().as_bytes());
    assert_eq!(reply["ok"], true, "parse failed: {reply}");
    reply["result"]["model"].clone()
}

fn font() -> Vec<u8> {
    // '√' shares glyph 2, whose MATH table carries the vertical construction.
    support::FontBuilder::new(&[('x', 1), ('+', 2), ('1', 3), ('√', 2)], 250, true).finish()
}

fn font_entry(font: &[u8]) -> Value {
    json!({
        "id": "main",
        "sha256": format!("{:x}", Sha256::digest(font)),
        "bytes_base64": STANDARD.encode(font),
        "face_index": 0,
    })
}

fn render_request(formula: &Value, font: Option<&[u8]>) -> Vec<u8> {
    let mut options = json!({});
    if let Some(font) = font {
        options["fonts"] = json!([font_entry(font)]);
        options["fallback_order"] = json!(["main"]);
    }
    let contract = json!({
        "origin": FORMULA_CONTRACT.origin,
        "revision": FORMULA_CONTRACT.revision,
        "schema_path": FORMULA_CONTRACT.definition_path,
        "sha256": FORMULA_CONTRACT.sha256,
    });
    json!({
        "abi_version": 1, "schema_version": 1,
        "op": "render", "formula": formula,
        "formula_contract": contract, "output": "svg",
        "options": options,
    })
    .to_string()
    .into_bytes()
}

#[test]
fn render_renders_a_parsed_formula_to_svg() {
    let model = parse_model();
    let font = font();
    let reply = request_json(&render_request(&model, Some(&font)));
    assert_eq!(reply["ok"], true, "render failed: {reply}");
    let svg = reply["result"]["svg"].as_str().expect("svg string");
    assert!(svg.starts_with("<svg"), "svg root missing: {svg}");
    // Real glyph outlines, not boxes: x+1 carries one path per glyph. The
    // fixture triangle (0,-200)(500,700)(0,-200) at 24px/1000upem flips to
    // top-left coordinates around the row baseline (16.8px from y_max).
    assert_eq!(svg.matches("<path").count(), 3, "three glyph paths: {svg}");
    assert!(
        svg.contains("<g id=\"glyphs\"><path d=\"M0 21.6 L12 0 L0 21.6 Z\"/>"),
        "the x glyph path is wrong: {svg}"
    );
}

#[test]
fn render_requires_the_current_contract_pin() {
    let model = parse_model();
    let font = font();
    let mut request = render_request(&model, Some(&font));
    let mut value: Value = serde_json::from_slice(&request).unwrap();
    value["formula_contract"]["origin"] = json!("other");
    request = value.to_string().into_bytes();
    let reply = request_json(&request);
    assert_eq!(reply["error"]["code"], "UNSUPPORTED_CONTRACT");
}

#[test]
fn render_refuses_without_fonts() {
    let model = parse_model();
    let reply = request_json(&render_request(&model, None));
    assert_eq!(reply["error"]["code"], "MISSING_FONT");
}

#[test]
fn render_refuses_a_corrupt_font() {
    let model = parse_model();
    let mut font = font();
    font[200] ^= 0xff;
    let reply = request_json(&render_request(&model, Some(&font)));
    assert_eq!(reply["error"]["code"], "INVALID_FONT");
}

#[test]
fn render_lays_out_a_sqrt_through_the_wire() {
    let request = json!({
        "abi_version": 1, "schema_version": 1,
        "op": "parse", "format": "mathml",
        "source": "<math><msqrt><mi>x</mi></msqrt></math>",
    });
    let reply = request_json(request.to_string().as_bytes());
    assert_eq!(reply["ok"], true, "parse failed: {reply}");
    let model = reply["result"]["model"].clone();
    let font = font();
    let rendered = request_json(&render_request(&model, Some(&font)));
    assert_eq!(rendered["ok"], true, "render failed: {rendered}");
    let svg = rendered["result"]["svg"].as_str().expect("svg string");
    assert_eq!(svg.matches("<path").count(), 2, "two glyph paths: {svg}");
    // The surd variant (glyph 3) hangs below the row baseline (27.36px =
    // 16.8 ink + 3.48 gap + 3.528 rule + 3.552 extra ascender); its piece
    // baseline sits one slot bottom lower, and the rule overlays 'x'.
    let glyphs_start = svg.find("<g id=\"glyphs\">").expect("glyph layer");
    let layer = &svg[glyphs_start..];
    assert_eq!(
        layer,
        concat!(
            "<g id=\"glyphs\"><path d=\"M0 37.152 L12 15.552000000000003 L0 37.152 Z\"/>",
            "<path d=\"M14.4 32.16 L26.4 10.559999999999999 L14.4 32.16 Z\"/>",
            "</g></g></svg>",
        ),
        "glyph layer bytes: {svg}"
    );
}

#[test]
fn render_lays_out_a_subscript_through_the_wire() {
    let request = json!({
        "abi_version": 1, "schema_version": 1,
        "op": "parse", "format": "mathml",
        "source": "<math><msub><mi>x</mi><mn>1</mn></msub></math>",
    });
    let reply = request_json(request.to_string().as_bytes());
    assert_eq!(reply["ok"], true, "parse failed: {reply}");
    let model = reply["result"]["model"].clone();
    let font = font();
    let rendered = request_json(&render_request(&model, Some(&font)));
    assert_eq!(rendered["ok"], true, "render failed: {rendered}");
    let svg = rendered["result"]["svg"].as_str().expect("svg string");
    assert_eq!(svg.matches("<path").count(), 2, "two glyph paths: {svg}");
    // The subscript must sit below the base: pin the glyph layer exactly.
    // Script '1' lays out at 19.2px (80%); its baseline drops 10.92px below
    // the row's (16.8 + ink ascent 13.44 - TopMax 2.52) and trails the base
    // advance with SpaceAfterScript (2.712px) after it.
    let glyphs_start = svg.find("<g id=\"glyphs\">").expect("glyph layer");
    let layer = &svg[glyphs_start..];
    assert_eq!(
        layer,
        concat!(
            "<g id=\"glyphs\"><path d=\"M0 21.6 L12 0 L0 21.6 Z\"/>",
            "<path d=\"M14.4 31.560000000000002 L24 14.280000000000001 L14.4 31.560000000000002 Z\"/>",
            "</g></g></svg>",
        ),
        "glyph layer bytes: {svg}"
    );
}
