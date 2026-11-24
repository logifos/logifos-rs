//! The versioned C ABI: owned request/response buffers, no retained state.
//! Every runtime transport (Ruby, WASM, Python) enters through the same
//! JSON semantics this ABI wraps.

use std::slice;

/// # Safety
///
/// `request` must point to `request_len` bytes of valid UTF-8 and remain
/// readable for the duration of the call. The returned buffer is owned by
/// the caller and released with [`logifos_reply_free`].
#[no_mangle]
pub unsafe extern "C" fn logifos_request_json(
    request: *const u8,
    request_len: usize,
    reply_len: *mut usize,
) -> *mut u8 {
    let reply = std::panic::catch_unwind(|| {
        let request = slice::from_raw_parts(request, request_len);
        crate::request_bytes(request)
    })
    .unwrap_or_else(|_| b"{\"ok\":false,\"error\":{\"code\":\"PANIC\"}}".to_vec());
    let mut owned = reply.into_boxed_slice();
    let len = owned.len();
    let pointer = owned.as_mut_ptr();
    std::mem::forget(owned);
    unsafe { *reply_len = len };
    pointer
}

/// # Safety
///
/// Releases a buffer returned by [`logifos_request_json`] with the same
/// length it was returned with.
#[no_mangle]
pub unsafe extern "C" fn logifos_reply_free(reply: *mut u8, reply_len: usize) {
    if reply.is_null() {
        return;
    }
    drop(unsafe { Vec::from_raw_parts(reply, reply_len, reply_len) });
}

#[no_mangle]
pub extern "C" fn logifos_abi_version() -> u32 {
    crate::ABI_VERSION
}

#[no_mangle]
pub extern "C" fn logifos_schema_version() -> u32 {
    crate::SCHEMA_VERSION
}
