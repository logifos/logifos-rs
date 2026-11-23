//! Unpublished WASM producer for the shared facade's UTF-8 JSON wire protocol.

use wasm_bindgen::prelude::*;

/// Borrows bytes for one call and returns a separate owned UTF-8 JSON response.
/// Bindgen copies the result into a JavaScript Uint8Array and frees Rust storage.
/// No caller bytes or options survive a call. Engine operations currently refuse.
///
/// WASM uses aborting panics: a trap never unwinds into JavaScript. Allocation
/// exhaustion or an unexpected trap invalidates the instance; recreate it.
#[wasm_bindgen]
pub fn request_json(bytes: &[u8]) -> Vec<u8> {
    logifos_facade::request_bytes(bytes)
}

/// Allows JavaScript to reject excessive input before bindgen copies it.
#[wasm_bindgen]
pub fn max_request_bytes() -> usize {
    logifos_facade::MAX_REQUEST_BYTES
}
