//! The request facade: every runtime (Ruby, WASM, Python) enters through
//! this JSON boundary, the same contract plurimath-rs exposes. Ops are
//! explicit; every request carries its options; results are owned.

use serde_json::{json, Value};

/// Handles one facade request. Unknown ops and formats refuse typed —
/// the boundary never panics.
pub fn request_json(bytes: &[u8]) -> Value {
    match handle(bytes) {
        Ok(value) => json!({"ok": true, "result": value}),
        Err(error) => json!({"ok": false, "error": error}),
    }
}

fn handle(bytes: &[u8]) -> Result<Value, String> {
    let request = logifos_json::decode(bytes)?;
    let op = request
        .get("op")
        .and_then(Value::as_str)
        .ok_or("request.op must be a string")?;
    match op {
        "render" => Err("renderer is not implemented".into()),
        "parse" => Err(format!(
            "format {:?} has no parser yet (lands in P1)",
            request.get("format").and_then(Value::as_str).unwrap_or("?")
        )),
        other => Err(format!("unknown op {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_unknown_ops_typed() {
        let reply = request_json(br#"{"op":"nope"}"#);
        assert_eq!(reply["ok"], false);
        assert!(reply["error"].as_str().unwrap().contains("unknown op"));
    }

    #[test]
    fn refuses_missing_formats_typed() {
        let reply = request_json(br#"{"op":"parse"}"#);
        assert_eq!(reply["ok"], false);
    }

    #[test]
    fn reports_unimplemented_renderer() {
        let reply = request_json(br#"{"op":"render"}"#);
        assert_eq!(reply["ok"], false);
        assert_eq!(reply["error"], "renderer is not implemented");
    }
}
