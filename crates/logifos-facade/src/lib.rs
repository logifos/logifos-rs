//! Shared JSON request behavior. Native and other transports enter here.
//! Only capability negotiation is implemented; engine operations refuse.

use serde_json::{json, Map, Value};

/// Transport ownership version, independent of the package version.
pub const ABI_VERSION: u32 = 1;
/// JSON envelope version, independent of the package version.
pub const SCHEMA_VERSION: u32 = 1;
/// Encoded response hard ceiling.
pub const MAX_RESPONSE_BYTES: usize = 8 << 20;

const LIMITS: [(&str, usize); 10] = [
    ("request_bytes", logifos_json::MAX_BYTES),
    ("json_depth", logifos_json::MAX_DEPTH),
    ("source_bytes", 1 << 20),
    ("tree_nodes", 100_000),
    ("tree_depth", 128),
    ("font_count", 16),
    ("font_bytes", 4 << 20),
    ("total_font_bytes", 4 << 20),
    ("response_bytes", MAX_RESPONSE_BYTES),
    ("glyphs", 100_000),
];

struct Failure {
    code: &'static str,
    path: String,
    message: &'static str,
}

fn fail(code: &'static str, path: &str, message: &'static str) -> Failure {
    Failure {
        code,
        path: path.into(),
        message,
    }
}

fn error(code: &str, path: &str, message: &str, stage: &str) -> Value {
    json!({"abi_version": ABI_VERSION, "schema_version": SCHEMA_VERSION,
        "ok": false, "error": {"code": code, "path": path, "message": message, "stage": stage}})
}

/// Handles one UTF-8 JSON request with bounded structured failures.
/// Successful negotiation does not qualify parsing, fonts, models or rendering.
pub fn request_json(bytes: &[u8]) -> Value {
    let request =
        match logifos_json::decode_checked(bytes, logifos_json::MAX_BYTES, logifos_json::MAX_DEPTH)
        {
            Ok(request) => request,
            Err(reason) => return error(reason.code, "", reason.message, "decode"),
        };
    match handle(&request, bytes) {
        Ok(reply) => reply,
        Err(reason) => error(reason.code, &reason.path, reason.message, "contract"),
    }
}

fn handle(request: &Value, bytes: &[u8]) -> Result<Value, Failure> {
    let object = request
        .as_object()
        .ok_or_else(|| fail("INVALID_REQUEST", "", "request must be an object"))?;
    for (name, code) in [
        ("abi_version", "UNSUPPORTED_ABI"),
        ("schema_version", "UNSUPPORTED_SCHEMA"),
    ] {
        if let Some(version) = object.get(name) {
            if version.as_u64() != Some(1) {
                return Err(fail(code, &format!("/{name}"), "unsupported version"));
            }
        } else {
            return Err(fail(
                "INVALID_REQUEST",
                &format!("/{name}"),
                "version is required",
            ));
        }
    }
    let op = object
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| fail("INVALID_REQUEST", "/op", "operation is required"))?;
    let fields: &[&str] = match op {
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
        _ => return Err(fail("INVALID_REQUEST", "/op", "unknown operation")),
    };
    closed(
        object,
        &[&["abi_version", "schema_version", "op", "options"], fields].concat(),
        "",
        "INVALID_REQUEST",
    )?;
    for field in fields {
        if !object.contains_key(*field) {
            return Err(fail(
                "INVALID_REQUEST",
                &format!("/{field}"),
                "operation field is required",
            ));
        }
    }
    if matches!(op, "parse" | "transform") && request["format"] != "mathml" {
        return Err(fail("INVALID_REQUEST", "/format", "unsupported format"));
    }
    if op == "parse" && !request["source"].is_string() {
        return Err(fail(
            "INVALID_REQUEST",
            "/source",
            "source must be a string",
        ));
    }
    if op == "transform" {
        if request["direction"] != "syntax_to_formula"
            && request["direction"] != "formula_to_syntax"
        {
            return Err(fail(
                "INVALID_REQUEST",
                "/direction",
                "invalid transform direction",
            ));
        }
        if !request["input"].is_object() {
            return Err(fail("INVALID_REQUEST", "/input", "input must be an object"));
        }
        identity(&request["input_contract"], "/input_contract")?;
        identity(&request["output_contract"], "/output_contract")?;
    }
    if op == "render" {
        if !request["formula"].is_object() {
            return Err(fail(
                "INVALID_REQUEST",
                "/formula",
                "formula must be an object",
            ));
        }
        identity(&request["formula_contract"], "/formula_contract")?;
        if request["output"] != "svg" && request["output"] != "png" {
            return Err(fail("INVALID_REQUEST", "/output", "invalid output format"));
        }
    }
    let mut limits: Map<String, Value> = LIMITS
        .iter()
        .map(|(key, value)| ((*key).into(), json!(value)))
        .collect();
    let mut resources_supplied = false;
    if let Some(options) = object.get("options") {
        let options = options
            .as_object()
            .ok_or_else(|| fail("INVALID_REQUEST", "/options", "options must be an object"))?;
        closed(
            options,
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
        if let Some(value) = options.get("font_size_px") {
            if !value.as_f64().is_some_and(|n| n > 0.0 && n <= 4096.0) {
                return Err(fail(
                    "INVALID_REQUEST",
                    "/options/font_size_px",
                    "invalid font size",
                ));
            }
        }
        if let Some(value) = options.get("display") {
            if value != "inline" && value != "block" {
                return Err(fail(
                    "INVALID_REQUEST",
                    "/options/display",
                    "invalid display mode",
                ));
            }
        }
        if let Some(value) = options.get("color") {
            if !value.as_str().is_some_and(|s| {
                s.len() == 7
                    && s.starts_with('#')
                    && s.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
            }) {
                return Err(fail(
                    "INVALID_REQUEST",
                    "/options/color",
                    "invalid RGB color",
                ));
            }
        }
        for name in ["fonts", "fallback_order"] {
            if let Some(value) = options.get(name) {
                let values = value.as_array().ok_or_else(|| {
                    fail(
                        "INVALID_REQUEST",
                        &format!("/options/{name}"),
                        "resource list must be an array",
                    )
                })?;
                if values.len() > 16 {
                    return Err(fail(
                        "INVALID_REQUEST",
                        &format!("/options/{name}"),
                        "too many resources",
                    ));
                }
                resources_supplied |= !values.is_empty();
            }
        }
        if let Some(value) = options.get("limits") {
            let requested = value.as_object().ok_or_else(|| {
                fail(
                    "INVALID_REQUEST",
                    "/options/limits",
                    "limits must be an object",
                )
            })?;
            for (name, value) in requested {
                let ceiling = LIMITS
                    .iter()
                    .find(|(key, _)| key == name)
                    .map(|(_, value)| *value as u64);
                if !value
                    .as_u64()
                    .zip(ceiling)
                    .is_some_and(|(n, ceiling)| n > 0 && n <= ceiling)
                {
                    return Err(fail(
                        "INVALID_REQUEST",
                        "/options/limits",
                        "invalid or raised limit",
                    ));
                }
                limits.insert(name.clone(), value.clone());
            }
        }
    }
    let limit = |name: &str| limits[name].as_u64().unwrap_or(0) as usize;
    logifos_json::decode_checked(bytes, limit("request_bytes"), limit("json_depth"))
        .map_err(|reason| fail(reason.code, "/options/limits", reason.message))?;
    if op == "render" && request["output"] == "png" {
        return Err(fail(
            "UNSUPPORTED_OUTPUT",
            "/output",
            "PNG is unavailable in the core",
        ));
    }
    if request["source"]
        .as_str()
        .is_some_and(|s| s.len() > limit("source_bytes"))
    {
        return Err(fail(
            "RESOURCE_LIMIT",
            "/source",
            "source byte limit exceeded",
        ));
    }
    // Do not claim font identity/validity or external schema qualification.
    if resources_supplied {
        return Err(fail(
            "NOT_IMPLEMENTED",
            "/options/fonts",
            "font resource validation is not implemented",
        ));
    }
    if op != "negotiate" {
        return Err(fail(
            "NOT_IMPLEMENTED",
            "/op",
            "operation is not implemented",
        ));
    }
    let reply = json!({"abi_version": ABI_VERSION, "schema_version": SCHEMA_VERSION, "ok": true,
        "result": {"kind": "capabilities", "abi_versions": [ABI_VERSION], "schema_versions": [SCHEMA_VERSION],
        "implemented_operations": [], "outputs": [], "formula_contract": null, "syntax_contract": null, "limits": limits}});
    if reply.to_string().len() > limit("response_bytes") {
        return Err(fail(
            "RESOURCE_LIMIT",
            "/options/limits/response_bytes",
            "response cannot fit requested limit",
        ));
    }
    Ok(reply)
}

fn closed(
    object: &Map<String, Value>,
    allowed: &[&str],
    path: &str,
    code: &'static str,
) -> Result<(), Failure> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(fail(code, path, "unknown field"));
    }
    Ok(())
}

fn identity(value: &Value, path: &str) -> Result<(), Failure> {
    let object = value.as_object().ok_or_else(|| {
        fail(
            "INVALID_REQUEST",
            path,
            "contract identity must be an object",
        )
    })?;
    closed(
        object,
        &["origin", "revision", "schema_path", "sha256"],
        path,
        "INVALID_REQUEST",
    )?;
    let hex = |name: &str, len: usize| {
        object.get(name).and_then(Value::as_str).is_some_and(|s| {
            s.len() == len
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
    };
    if !hex("revision", 40)
        || !hex("sha256", 64)
        || ["origin", "schema_path"].iter().any(|name| {
            object
                .get(*name)
                .and_then(Value::as_str)
                .is_none_or(|s| s.is_empty())
        })
    {
        return Err(fail("INVALID_REQUEST", path, "invalid contract identity"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negotiation_advertises_transport_only() {
        let reply = request_json(br#"{"abi_version":1,"schema_version":1,"op":"negotiate"}"#);
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["result"]["outputs"], json!([]));
        assert_eq!(reply["result"]["implemented_operations"], json!([]));
        assert!(reply["result"]["formula_contract"].is_null());
    }
    #[test]
    fn structured_refusals_do_not_echo_caller_data() {
        for (input, code) in [
            (&b"{"[..], "INVALID_JSON"),
            (b"\xff", "INVALID_UTF8"),
            (br#"{"abi_version":2,"schema_version":1,"op":"negotiate"}"#, "UNSUPPORTED_ABI"),
            (br#"{"abi_version":1,"schema_version":2,"op":"negotiate"}"#, "UNSUPPORTED_SCHEMA"),
            (br#"{"abi_version":1,"schema_version":1,"op":"private-value"}"#, "INVALID_REQUEST"),
            (br#"{"abi_version":1,"schema_version":1,"op":"negotiate","options":{"private-value":true}}"#, "UNKNOWN_OPTION"),
            (br#"{"abi_version":1,"schema_version":1,"op":"parse","format":"mathml","source":"\u03b1"}"#, "NOT_IMPLEMENTED"),
        ] {
            let reply = request_json(input);
            assert_eq!(reply["ok"], false);
            assert_eq!(reply["error"]["code"], code);
            assert!(!reply.to_string().contains("private-value"));
        }
    }
    #[test]
    fn per_call_limits_do_not_leak() {
        let limited = request_json(br#"{"abi_version":1,"schema_version":1,"op":"negotiate","options":{"limits":{"response_bytes":10}}}"#);
        assert_eq!(limited["error"]["code"], "RESOURCE_LIMIT");
        let normal = request_json(br#"{"abi_version":1,"schema_version":1,"op":"negotiate"}"#);
        assert_eq!(normal["ok"], true);
        assert_eq!(
            normal["result"]["limits"]["response_bytes"],
            MAX_RESPONSE_BYTES
        );
    }
}
