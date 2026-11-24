//! Shared versioned JSON boundary for native and WASM transports.
//! Only negotiation is implemented; parse, transform and rendering refuse.

mod wire;

use serde_json::{json, Value};
use logifos_model::FORMULA_CONTRACT;
use wire::{Failure, Limits};

/// Effective transport ceiling; checked before JSON allocation.
pub const MAX_REQUEST_BYTES: usize = 262_144;
/// Transport ownership version, independent of the package version.
pub const ABI_VERSION: u32 = 1;
/// JSON envelope version, independent of the package version.
pub const SCHEMA_VERSION: u32 = 1;
/// Encoded response hard ceiling shared by both transports.
pub const MAX_RESPONSE_BYTES: usize = 16_384;

/// Handles a length-delimited UTF-8 v1 request without retaining caller state.
/// All decoding, validation and refusal categories are shared across transports.
pub fn request_json(bytes: &[u8]) -> Value {
    let result = logifos_json::decode_checked(bytes, MAX_REQUEST_BYTES, 32)
        .map_err(|error| Failure::new(error.code, "", "decode"))
        .and_then(|request| wire::handle(&request, bytes));
    match result {
        Ok(result) => json!({"abi_version":1,"schema_version":1,"ok":true,"result":result}),
        Err(error) => error.response(),
    }
}

/// Returns an owned encoded response. A lowered response ceiling is applied by
/// validation; if even the error cannot fit, the minimal typed refusal wins.
pub fn request_bytes(bytes: &[u8]) -> Vec<u8> {
    request_json(bytes).to_string().into_bytes()
}

/// Effective limits advertised by both transports.
pub fn limits() -> Value {
    Limits::default().as_value()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiation_advertises_parse_and_render() {
        let reply = request_json(br#"{"abi_version":1,"schema_version":1,"op":"negotiate"}"#);
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["result"]["outputs"], json!(["svg"]));
        assert_eq!(
            reply["result"]["implemented_operations"],
            json!(["negotiate", "parse", "render"])
        );
        assert_eq!(
            reply["result"]["formula_contract"]["origin"],
            FORMULA_CONTRACT.origin
        );
        assert!(reply["result"]["syntax_contract"].is_null());
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
            (br#"{"abi_version":1,"schema_version":1,"op":"parse","format":"mathml","source":"\u03b1"}"#, "INVALID_SOURCE"),
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

    #[test]
    fn negotiates_with_render_support() {
        let reply = request_json(br#"{"abi_version":1,"schema_version":1,"op":"negotiate"}"#);
        assert_eq!(reply["ok"], true);
        assert!(reply["result"]["implemented_operations"]
            .as_array()
            .is_some_and(|ops| ops.contains(&json!("render"))));
        assert_eq!(reply["result"]["outputs"], json!(["svg"]));
    }

    #[test]
    fn refuses_versionless_requests_and_unknown_operations() {
        assert_eq!(
            request_json(br#"{"op":"render"}"#)["error"]["code"],
            "INVALID_REQUEST"
        );
        assert_eq!(
            request_json(br#"{"abi_version":2,"schema_version":1,"op":"negotiate"}"#)["error"]
                ["code"],
            "UNSUPPORTED_ABI"
        );
        assert_eq!(
            request_json(br#"{"abi_version":1,"schema_version":1,"op":"unknown"}"#)["error"]
                ["code"],
            "INVALID_REQUEST"
        );
    }

    #[test]
    fn native_bytes_use_the_identical_wire_boundary() {
        let request = br#"{"abi_version":1,"schema_version":1,"op":"parse","format":"mathml","source":"<math><mi>x</mi></math>"}"#;
        assert_eq!(
            request_bytes(request),
            request_json(request).to_string().into_bytes()
        );
        assert_eq!(request_json(request)["ok"], true);
        assert_eq!(
            request_json(request)["result"]["model"]["class"],
            "Math::Formula"
        );
    }
}
