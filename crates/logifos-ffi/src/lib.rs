//! Native transport for the JSON facade. Rendering behavior belongs to the facade.

use std::ffi::{c_char, CString};
use std::panic::catch_unwind;
use std::ptr;

/// Borrows a JSON request for this call and returns an owned, NUL-terminated reply.
///
/// Release each non-null reply exactly once with [`logifos_string_free`]. Null
/// input, an unrepresentable length, or a caught panic returns null. Otherwise,
/// errors (including unimplemented rendering) are JSON replies from the facade.
///
/// # Safety
///
/// When `len <= isize::MAX`, a non-null `input` must reference `len` initialized,
/// readable bytes in one allocation, valid and unchanged throughout this call.
#[no_mangle]
pub unsafe extern "C" fn logifos_request_json(input: *const u8, len: usize) -> *mut c_char {
    if input.is_null() || len > isize::MAX as usize {
        return ptr::null_mut();
    }

    catch_unwind(|| {
        // SAFETY: The caller guarantees a valid slice; null and length overflow
        // are rejected above. The facade retains no reference to these bytes.
        let bytes = unsafe { std::slice::from_raw_parts(input, len) };
        let reply = logifos_facade::request_json(bytes).to_string();
        CString::new(reply).map_or(ptr::null_mut(), CString::into_raw)
    })
    .unwrap_or(ptr::null_mut())
}

/// Releases a reply allocated by [`logifos_request_json`]. Null is a no-op.
///
/// # Safety
///
/// A non-null `reply` must be an unmodified pointer returned by this library's
/// `logifos_request_json`, still allocated and not previously freed. Its contents
/// must not have been modified. After this call, the pointer is invalid.
#[no_mangle]
pub unsafe extern "C" fn logifos_string_free(reply: *mut c_char) {
    if !reply.is_null() {
        // SAFETY: The caller transfers the original, live CString allocation.
        drop(unsafe { CString::from_raw(reply) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn replies_outlive_input_and_are_independently_owned() {
        let input = br#"{"op":"render"}"#.to_vec();
        // SAFETY: Input bytes remain valid throughout both calls.
        let first = unsafe { logifos_request_json(input.as_ptr(), input.len()) };
        let second = unsafe { logifos_request_json(input.as_ptr(), input.len()) };
        drop(input);
        assert!(!first.is_null());
        assert!(!second.is_null());
        assert_ne!(first, second);
        // SAFETY: Each result is live, NUL-terminated, and released once.
        unsafe {
            assert!(CStr::from_ptr(first)
                .to_str()
                .unwrap()
                .contains("renderer is not implemented"));
            logifos_string_free(first);
            assert!(CStr::from_ptr(second)
                .to_str()
                .unwrap()
                .contains("renderer is not implemented"));
            logifos_string_free(second);
        }
    }

    #[test]
    fn rejects_null_and_unrepresentable_lengths() {
        let byte = b' ';
        // SAFETY: These invalid arguments are rejected before dereferencing.
        unsafe {
            assert!(logifos_request_json(ptr::null(), 0).is_null());
            assert!(logifos_request_json(ptr::null(), 1).is_null());
            assert!(logifos_request_json(&byte, usize::MAX).is_null());
            logifos_string_free(ptr::null_mut());
        }
    }
}
