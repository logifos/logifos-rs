use base64::{engine::general_purpose::STANDARD, Engine};
use logifos_layout::font::{FontProvider, FontResource};
use logifos_layout::layout::{layout as layout_formula, LayoutOptions};
use logifos_model::{Formula, FORMULA_CONTRACT};
use logifos_svg::{emit as emit_svg, SvgOptions};
use logifos_syntax_mathml::{parse as parse_mathml, transform as transform_mathml};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// The formula contract identity, projected into the request identity shape.
fn formula_contract_value() -> Value {
    json!({
        "origin": FORMULA_CONTRACT.origin,
        "revision": FORMULA_CONTRACT.revision,
        "schema_path": FORMULA_CONTRACT.definition_path,
        "sha256": FORMULA_CONTRACT.sha256,
    })
}

/// The pinned identity must name exactly this workspace's formula contract.
fn contract_pinned(identity: &Value) -> bool {
    identity == &formula_contract_value()
}

pub(super) struct Failure {
    code: &'static str,
    path: String,
    stage: &'static str,
}
impl Failure {
    pub(super) fn new(code: &'static str, path: &str, stage: &'static str) -> Self {
        Self {
            code,
            path: path.into(),
            stage,
        }
    }
    pub(super) fn response(&self) -> Value {
        json!({"abi_version":1,"schema_version":1,"ok":false,"error":{
            "code":self.code,"message":"request refused","path":self.path,"stage":self.stage}})
    }
}
type Result<T> = std::result::Result<T, Failure>;
fn fail(code: &'static str, path: &str) -> Failure {
    Failure::new(code, path, "contract")
}
fn invalid(path: &str) -> Failure {
    fail("INVALID_REQUEST", path)
}
fn object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>> {
    value.as_object().ok_or_else(|| invalid(path))
}
fn closed(
    value: &Map<String, Value>,
    fields: &[&str],
    path: &str,
    code: &'static str,
) -> Result<()> {
    if value.keys().any(|key| !fields.contains(&key.as_str())) {
        return Err(fail(code, path));
    }
    Ok(())
}
fn text<'a>(value: &'a Value, path: &str) -> Result<&'a str> {
    value.as_str().ok_or_else(|| invalid(path))
}
fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

const CEILINGS: &[(&str, u64)] = &[
    ("request_bytes", super::MAX_REQUEST_BYTES as u64),
    ("json_depth", 32),
    ("source_bytes", 65_536),
    ("tree_nodes", 100_000),
    ("tree_depth", 128),
    ("font_count", 16),
    ("font_bytes", 131_072),
    ("total_font_bytes", 131_072),
    ("response_bytes", super::MAX_RESPONSE_BYTES as u64),
    ("glyphs", 100_000),
];
pub(super) struct Limits(Value);
impl Default for Limits {
    fn default() -> Self {
        Self(
            CEILINGS
                .iter()
                .map(|(key, value)| ((*key).into(), json!(value)))
                .collect::<Map<_, _>>()
                .into(),
        )
    }
}
impl Limits {
    pub(super) fn as_value(&self) -> Value {
        self.0.clone()
    }
    fn get(&self, name: &str) -> usize {
        self.0[name].as_u64().unwrap_or(0) as usize
    }
    fn lowered(value: Option<&Value>) -> Result<Self> {
        let mut limits = Self::default();
        if let Some(value) = value {
            for (name, value) in object(value, "/options/limits")? {
                let maximum = CEILINGS
                    .iter()
                    .find(|(key, _)| key == name)
                    .map(|(_, value)| *value)
                    .ok_or_else(|| invalid("/options/limits"))?;
                let number = value
                    .as_u64()
                    .filter(|&n| n > 0 && n <= maximum)
                    .ok_or_else(|| invalid("/options/limits"))?;
                limits.0[name] = json!(number);
            }
        }
        Ok(limits)
    }
}
fn identity(value: &Value, path: &str) -> Result<()> {
    let map = object(value, path)?;
    closed(
        map,
        &["origin", "revision", "schema_path", "sha256"],
        path,
        "INVALID_REQUEST",
    )?;
    for name in ["origin", "schema_path"] {
        if text(&value[name], path)?.is_empty() {
            return Err(invalid(path));
        }
    }
    if !hex(text(&value["revision"], path)?, 40) || !hex(text(&value["sha256"], path)?, 64) {
        return Err(invalid(path));
    }
    Ok(())
}

pub(super) fn handle(request: &Value, bytes: &[u8]) -> Result<Value> {
    let envelope = object(request, "")?;
    for (name, code) in [
        ("abi_version", "UNSUPPORTED_ABI"),
        ("schema_version", "UNSUPPORTED_SCHEMA"),
    ] {
        match envelope.get(name) {
            None => return Err(invalid(&format!("/{name}"))),
            Some(value) if value.as_u64() == Some(1) => {}
            _ => return Err(fail(code, &format!("/{name}"))),
        }
    }
    let operation = text(&request["op"], "/op")?;
    let extra: &[&str] = match operation {
        "negotiate" => &[],
        "parse" => &["format", "source"],
        "transform" => &[
            "format",
            "direction",
            "input",
            "input_contract",
            "output_contract",
        ],
        "render" => &["formula", "formula_contract", "output"],
        _ => return Err(invalid("/op")),
    };
    let mut fields = vec!["abi_version", "schema_version", "op", "options"];
    fields.extend(extra);
    closed(envelope, &fields, "", "INVALID_REQUEST")?;
    for field in extra {
        if !envelope.contains_key(*field) {
            return Err(invalid(&format!("/{field}")));
        }
    }
    match operation {
        "parse" | "transform" => {
            if request["format"] != "mathml" {
                return Err(invalid("/format"));
            }
            if operation == "parse" {
                text(&request["source"], "/source")?;
            } else {
                if !["syntax_to_formula", "formula_to_syntax"]
                    .contains(&text(&request["direction"], "/direction")?)
                {
                    return Err(invalid("/direction"));
                }
                object(&request["input"], "/input")?;
                identity(&request["input_contract"], "/input_contract")?;
                identity(&request["output_contract"], "/output_contract")?;
            }
        }
        "render" => {
            object(&request["formula"], "/formula")?;
            identity(&request["formula_contract"], "/formula_contract")?;
            if !["svg", "png"].contains(&text(&request["output"], "/output")?) {
                return Err(invalid("/output"));
            }
        }
        _ => {}
    }
    let empty = json!({});
    let options = envelope.get("options").unwrap_or(&empty);
    let options_map = object(options, "/options")?;
    closed(
        options_map,
        &[
            "limits",
            "font_size_px",
            "display",
            "color",
            "fonts",
            "fallback_order",
        ],
        "/options",
        "UNKNOWN_OPTION",
    )?;
    if let Some(size) = options_map.get("font_size_px") {
        if !size
            .as_f64()
            .is_some_and(|n| n.is_finite() && n > 0.0 && n <= 4096.0)
        {
            return Err(invalid("/options/font_size_px"));
        }
    }
    if let Some(display) = options_map.get("display") {
        if ![json!("inline"), json!("block")].contains(display) {
            return Err(invalid("/options/display"));
        }
    }
    if let Some(color) = options_map.get("color") {
        if !color.as_str().is_some_and(|s| {
            s.len() == 7
                && s.starts_with('#')
                && s.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
        }) {
            return Err(invalid("/options/color"));
        }
    }
    font_envelopes(options)?;
    let limits = Limits::lowered(options_map.get("limits"))?;
    let outcome = execute(request, bytes, operation, options, &limits);
    let encoded_size = match &outcome {
        Ok(result) => json!({"abi_version":1,"schema_version":1,"ok":true,"result":result}),
        Err(error) => error.response(),
    }
    .to_string()
    .len();
    if encoded_size > limits.get("response_bytes") {
        return Err(fail("RESOURCE_LIMIT", ""));
    }
    outcome
}

fn execute(
    request: &Value,
    bytes: &[u8],
    operation: &str,
    options: &Value,
    limits: &Limits,
) -> Result<Value> {
    if bytes.len() > limits.get("request_bytes") {
        return Err(fail("RESOURCE_LIMIT", ""));
    }
    logifos_json::check_depth(bytes, limits.get("json_depth"))
        .map_err(|_| fail("RESOURCE_LIMIT", "/options/limits/json_depth"))?;
    if operation == "render" && request["output"] == "png" {
        return Err(fail("UNSUPPORTED_OUTPUT", "/output"));
    }
    if request
        .get("source")
        .and_then(Value::as_str)
        .is_some_and(|source| source.len() > limits.get("source_bytes"))
    {
        return Err(fail("RESOURCE_LIMIT", "/source"));
    }
    let font_count = fonts(options, limits)?;
    let result = match operation {
        "negotiate" => json!({"kind":"capabilities", "abi_versions":[1],"schema_versions":[1],
            "implemented_operations":["negotiate","parse","render"],"outputs":["svg"],
            "formula_contract":formula_contract_value(),"syntax_contract":null,
            "limits":limits.as_value()}),
        "parse" => {
            let source = text(&request["source"], "/source")?;
            let document = parse_mathml(source).map_err(|_| fail("INVALID_SOURCE", "/source"))?;
            let formula =
                transform_mathml(&document).map_err(|_| fail("TRANSFORM_FAILED", "/source"))?;
            let model = formula
                .to_wire()
                .map_err(|_| fail("WIRE_FAILED", "/source"))?;
            return Ok(json!({"model": model, "formula_contract": formula_contract_value()}));
        }
        "render" if font_count == 0 => return Err(fail("MISSING_FONT", "/options/fonts")),
        "render" => {
            if !contract_pinned(&request["formula_contract"]) {
                return Err(fail("UNSUPPORTED_CONTRACT", "/formula_contract"));
            }
            let formula = Formula::from_wire(
                request["formula"]
                    .as_object()
                    .ok_or_else(|| fail("INVALID_REQUEST", "/formula"))?,
            )
            .map_err(|_| fail("INVALID_FORMULA", "/formula"))?;
            let mut owned: Vec<(String, String, Vec<u8>, u16)> = Vec::new();
            for (index, font) in options
                .get("fonts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .enumerate()
            {
                let path = format!("/options/fonts/{index}");
                let data = STANDARD
                    .decode(text(&font["bytes_base64"], &path)?)
                    .map_err(|_| fail("INVALID_FONT", &path))?;
                owned.push((
                    text(&font["id"], &path)?.to_owned(),
                    text(&font["sha256"], &path)?.to_owned(),
                    data,
                    font["face_index"].as_u64().unwrap_or(0) as u16,
                ));
            }
            let resources: Vec<FontResource> = owned
                .iter()
                .map(|(id, hash, bytes, face_index)| FontResource {
                    id,
                    sha256: hash,
                    bytes,
                    face_index: *face_index,
                })
                .collect();
            let order: Vec<&str> = options
                .get("fallback_order")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            let mut provider = FontProvider::new(&resources, &order)
                .map_err(|_| fail("INVALID_FONT", "/options/fonts"))?;
            let font_size_px = options
                .get("font_size_px")
                .and_then(Value::as_f64)
                .unwrap_or(24.0);
            let options_layout = LayoutOptions {
                font_size_px,
                script_level: 0,
            };
            let laid = layout_formula(&formula, &mut provider, options_layout)
                .map_err(|_e| fail("LAYOUT_FAILED", ""))?;
            let color = options
                .get("color")
                .and_then(Value::as_str)
                .unwrap_or("#000000");
            let document = emit_svg(
                &laid.tree,
                &laid.glyphs,
                &SvgOptions {
                    color: color.to_owned(),
                    title: None,
                },
            )
            .map_err(|_| fail("RENDER_FAILED", ""))?;
            return Ok(json!({"svg": document.as_str()}));
        }
        _ => {
            return Err(fail(
                "UNPINNED_CONTRACT",
                if request["direction"] == "syntax_to_formula" {
                    "/mathml_syntax"
                } else {
                    "/formula"
                },
            ))
        }
    };
    Ok(result)
}

// Envelope types precede limits and PNG refusal; byte identity follows them.
fn font_envelopes(options: &Value) -> Result<()> {
    for name in ["fonts", "fallback_order"] {
        if let Some(value) = options.get(name) {
            let array = value.as_array().ok_or_else(|| invalid("/options"))?;
            if array.len() > 16 {
                return Err(invalid("/options"));
            }
            if name == "fallback_order" && array.iter().any(|item| !item.is_string()) {
                return Err(invalid("/options/fallback_order"));
            }
        }
    }
    if let Some(resources) = options.get("fonts").and_then(Value::as_array) {
        for (index, font) in resources.iter().enumerate() {
            let path = format!("/options/fonts/{index}");
            closed(
                object(font, &path)?,
                &["id", "sha256", "bytes_base64", "face_index"],
                &path,
                "INVALID_REQUEST",
            )?;
            let id = text(&font["id"], &path)?;
            if id.is_empty()
                || id.len() > 64
                || !id.as_bytes()[0].is_ascii_alphabetic()
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                || !hex(text(&font["sha256"], &path)?, 64)
                || font["face_index"].as_u64().is_none_or(|n| n > 65_535)
                || text(&font["bytes_base64"], &path)?.len() > 5_592_408
            {
                return Err(invalid(&path));
            }
        }
    }
    Ok(())
}

fn fonts(options: &Value, limits: &Limits) -> Result<usize> {
    let no_fonts = Vec::new();
    let resources = match options.get("fonts") {
        Some(value) => value.as_array().ok_or_else(|| invalid("/options/fonts"))?,
        None => &no_fonts,
    };
    if resources.len() > limits.get("font_count") {
        return Err(fail("RESOURCE_LIMIT", "/options/fonts"));
    }
    let mut ids = BTreeSet::new();
    let mut total = 0;
    for (index, font) in resources.iter().enumerate() {
        let path = format!("/options/fonts/{index}");
        let id = text(&font["id"], &path)?;
        if !ids.insert(id) {
            return Err(fail("INVALID_FONT", &path));
        }
        let hash = text(&font["sha256"], &path)?;
        let encoded = text(&font["bytes_base64"], &path)?;
        let data = STANDARD
            .decode(encoded)
            .map_err(|_| fail("INVALID_FONT", &path))?;
        if STANDARD.encode(&data) != encoded {
            return Err(fail("INVALID_FONT", &path));
        }
        total += data.len();
        if data.len() > limits.get("font_bytes") || total > limits.get("total_font_bytes") {
            return Err(fail("RESOURCE_LIMIT", &path));
        }
        if format!("{:x}", Sha256::digest(&data)) != hash {
            return Err(fail("FONT_HASH_MISMATCH", &format!("{path}/sha256")));
        }
        if data.is_empty() {
            return Err(fail("INVALID_FONT", &path));
        }
    }
    let order = match options.get("fallback_order") {
        Some(value) => value
            .as_array()
            .ok_or_else(|| invalid("/options/fallback_order"))?,
        None => &no_fonts,
    };
    let ordered: BTreeSet<_> = order
        .iter()
        .map(|value| text(value, "/options/fallback_order"))
        .collect::<Result<_>>()?;
    if ordered != ids || ordered.len() != order.len() {
        return Err(fail("INVALID_FONT", "/options/fallback_order"));
    }
    Ok(resources.len())
}
