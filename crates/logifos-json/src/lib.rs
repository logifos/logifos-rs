//! Bounded, strict JSON decoding shared by runtime boundaries.

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;

/// Maximum encoded request size.
pub const MAX_BYTES: usize = 8 << 20;
/// Maximum container nesting, counting the root container as one.
pub const MAX_DEPTH: usize = 64;

/// A bounded diagnostic without caller content.
#[derive(Debug)]
pub struct DecodeError {
    /// Stable wire error category.
    pub code: &'static str,
    /// Diagnostic suitable for the public boundary.
    pub message: &'static str,
}

/// Decodes JSON using the shared hard ceilings.
pub fn decode(bytes: &[u8]) -> Result<Value, String> {
    decode_checked(bytes, MAX_BYTES, MAX_DEPTH).map_err(|error| error.message.into())
}

/// Decodes within caller-selected byte and container-depth ceilings.
pub fn decode_checked(
    bytes: &[u8],
    max_bytes: usize,
    max_depth: usize,
) -> Result<Value, DecodeError> {
    let fail = |code, message| DecodeError { code, message };
    if bytes.len() > max_bytes {
        return Err(fail("RESOURCE_LIMIT", "request byte limit exceeded"));
    }
    check_depth(bytes, max_depth)?;
    std::str::from_utf8(bytes).map_err(|_| fail("INVALID_UTF8", "request is not UTF-8"))?;
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = StrictValue::deserialize(&mut decoder).map_err(|_| {
        if unpaired_surrogate(bytes) {
            fail("INVALID_UTF8", "unpaired Unicode surrogate")
        } else {
            fail("INVALID_JSON", "invalid JSON or duplicate object key")
        }
    })?;
    decoder
        .end()
        .map_err(|_| fail("INVALID_JSON", "trailing bytes after JSON"))?;
    Ok(value.0)
}

/// Checks container depth before recursive decoding, ignoring string delimiters.
pub fn check_depth(bytes: &[u8], max_depth: usize) -> Result<(), DecodeError> {
    let fail = |code, message| DecodeError { code, message };
    // Scan before recursive deserialization, ignoring delimiters in strings.
    let (mut depth, mut quoted, mut escaped) = (0usize, false, false);
    for &byte in bytes {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else {
            match byte {
                b'"' => quoted = true,
                b'[' | b'{' => {
                    depth += 1;
                    if depth > max_depth {
                        return Err(fail("RESOURCE_LIMIT", "JSON depth limit exceeded"));
                    }
                }
                b']' | b'}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    Ok(())
}

fn unpaired_surrogate(bytes: &[u8]) -> bool {
    let quad = |offset: usize| {
        let digits = std::str::from_utf8(bytes.get(offset..offset + 4)?).ok()?;
        u16::from_str_radix(digits, 16).ok()
    };
    let (mut index, mut quoted) = (0, false);
    while index < bytes.len() {
        match bytes[index] {
            b'"' => quoted = !quoted,
            b'\\' if quoted => {
                if bytes.get(index + 1) == Some(&b'u') {
                    if let Some(code) = quad(index + 2) {
                        if (0xd800..=0xdbff).contains(&code) {
                            if bytes.get(index + 6..index + 8) != Some(b"\\u")
                                || !quad(index + 8)
                                    .is_some_and(|low| (0xdc00..=0xdfff).contains(&low))
                            {
                                return true;
                            }
                            index += 12;
                            continue;
                        }
                        if (0xdc00..=0xdfff).contains(&code) {
                            return true;
                        }
                        index += 6;
                        continue;
                    }
                }
                index += 2;
                continue;
            }
            _ => {}
        }
        index += 1;
    }
    false
}

struct StrictValue(Value);

impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct StrictVisitor;
        impl<'de> Visitor<'de> for StrictVisitor {
            type Value = StrictValue;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON value with unique object keys")
            }
            fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Bool(value)))
            }
            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Number(value.into())))
            }
            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Number(value.into())))
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
                Number::from_f64(value)
                    .map(|number| StrictValue(Value::Number(number)))
                    .ok_or_else(|| E::custom("nonfinite JSON number"))
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::String(value.into())))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(StrictValue(value)) = seq.next_element()? {
                    values.push(value);
                }
                Ok(StrictValue(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(de::Error::custom("duplicate object key"));
                    }
                    let StrictValue(value) = map.next_value()?;
                    values.insert(key, value);
                }
                Ok(StrictValue(Value::Object(values)))
            }
        }
        decoder.deserialize_any(StrictVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_unicode_and_rejects_ambiguous_json() {
        assert_eq!(decode(br#"{"source":"\u03b1"}"#).unwrap()["source"], "α");
        for input in [
            &b"{\"x\":1,\"x\":2}"[..],
            br#"{"nested":{"x":1,"\u0078":2}}"#,
            b"{}{}",
            b"\xef\xbb\xbf{}",
            b"[NaN]",
            b"[1e999]",
        ] {
            assert_eq!(
                decode_checked(input, MAX_BYTES, MAX_DEPTH)
                    .unwrap_err()
                    .code,
                "INVALID_JSON"
            );
        }
        for input in [&b"\xff"[..], br#""\ud800""#, br#""\udc00""#] {
            assert_eq!(
                decode_checked(input, MAX_BYTES, MAX_DEPTH)
                    .unwrap_err()
                    .code,
                "INVALID_UTF8"
            );
        }
    }
    #[test]
    fn limits_apply_before_recursive_decoding() {
        assert!(decode_checked(b"{\"x\":[{}]}", MAX_BYTES, 3).is_ok());
        assert_eq!(
            decode_checked(b"{\"x\":[{}]}", MAX_BYTES, 2)
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
        assert!(decode_checked(br#""[\"{]""#, MAX_BYTES, 1).is_ok());
        assert_eq!(
            decode_checked(b"{}", 1, MAX_DEPTH).unwrap_err().code,
            "RESOURCE_LIMIT"
        );
        let deep = [vec![b'['; MAX_DEPTH + 1], vec![b']'; MAX_DEPTH + 1]].concat();
        assert_eq!(
            decode_checked(&deep, MAX_BYTES, MAX_DEPTH)
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
    }
}
