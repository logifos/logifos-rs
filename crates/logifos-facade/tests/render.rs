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
    support::FontBuilder::new(&[('x', 1), ('+', 2), ('1', 3)], 250, true).finish()
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
    assert!(svg.contains('x'), "the x glyph is missing");
    assert!(svg.contains('+'), "the + glyph is missing");
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
