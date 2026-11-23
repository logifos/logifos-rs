//! Bounded JSON decoding for the logifos facade: every entry point parses
//! through this crate so depth and size limits are enforced in one place.

use serde_json::Value;

const MAX_DEPTH: usize = 64;
const MAX_BYTES: usize = 8 << 20;

/// Decodes JSON within the workspace's depth and size bounds.
pub fn decode(bytes: &[u8]) -> Result<Value, String> {
    if bytes.len() > MAX_BYTES {
        return Err(format!("request exceeds {} bytes", MAX_BYTES));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if depth_of(&value) > MAX_DEPTH {
        return Err(format!("request nesting exceeds {} levels", MAX_DEPTH));
    }
    Ok(value)
}

fn depth_of(v: &Value) -> usize {
    match v {
        Value::Array(items) => 1 + items.iter().map(depth_of).max().unwrap_or(0),
        Value::Object(map) => 1 + map.values().map(depth_of).max().unwrap_or(0),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_within_bounds() {
        assert!(decode(br#"{"op":"render"}"#).is_ok());
    }

    #[test]
    fn rejects_oversized_requests() {
        let big = vec![b'a'; MAX_BYTES + 1];
        assert!(decode(&big).is_err());
    }

    #[test]
    fn rejects_overly_nested_requests() {
        let mut value = String::new();
        for _ in 0..=MAX_DEPTH {
            value.push('[');
        }
        for _ in 0..=MAX_DEPTH {
            value.push(']');
        }
        assert!(decode(value.as_bytes()).is_err());
    }
}
