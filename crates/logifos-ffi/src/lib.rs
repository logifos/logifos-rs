//! Versioned native transport. All request semantics belong to logifos-facade.

#[cfg(panic = "abort")]
compile_error!("the native transport requires panic=unwind for containment");

use std::alloc::{alloc, dealloc, Layout};
use std::ffi::{c_char, CStr};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

/// A response allocation was returned, including JSON refusals.
pub const OK: u32 = 0;
/// Invalid transport arguments; no allocation was returned.
pub const INVALID_REQUEST: u32 = 1;
/// The response allocation failed; no allocation was returned.
pub const ALLOCATION_FAILED: u32 = 2;
/// A Rust panic was contained; no allocation was returned.
pub const INTERNAL_ERROR: u32 = 3;
/// Feature bit for the owned, length-delimited request transport.
pub const FEATURE_BUFFER_V1: u64 = 1;
/// Feature bit for shared versioned JSON negotiation and refusals.
pub const FEATURE_JSON_V1: u64 = 2;
/// Reserved render capability. Not advertised until rendering is implemented.
pub const FEATURE_RENDER: u64 = 4;

/// An owned allocation. Its bytes have no NUL terminator and must not be changed.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct LogifosBuffer {
    /// Pointer owned by the loaded library; null only when length is zero.
    pub ptr: *mut u8,
    /// Exact byte length required by the matching free.
    pub len: usize,
}

impl LogifosBuffer {
    const EMPTY: Self = Self {
        ptr: ptr::null_mut(),
        len: 0,
    };
}

/// Returns the transport ownership version.
#[no_mangle]
pub extern "C" fn logifos_abi_version() -> u32 {
    logifos_facade::ABI_VERSION
}

/// Returns the JSON envelope version.
#[no_mangle]
pub extern "C" fn logifos_schema_version() -> u32 {
    logifos_facade::SCHEMA_VERSION
}

/// Returns transport feature bits; RENDER advertises the facade's MathML-to-SVG op.
#[no_mangle]
pub extern "C" fn logifos_features() -> u64 {
    FEATURE_BUFFER_V1 | FEATURE_JSON_V1 | FEATURE_RENDER
}

/// Returns a static NUL-terminated package version; never free or modify it.
#[no_mangle]
pub extern "C" fn logifos_engine_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

fn contain<T>(work: impl FnOnce() -> T) -> Result<T, u32> {
    catch_unwind(AssertUnwindSafe(work)).map_err(|payload| {
        // A foreign panic payload may itself panic on drop. Do not allow a
        // second unwind to escape the boundary while handling the first one.
        std::mem::forget(payload);
        INTERNAL_ERROR
    })
}

fn allocate(
    bytes: &[u8],
    nul: bool,
    allocator: impl FnOnce(Layout) -> *mut u8,
) -> Result<LogifosBuffer, u32> {
    let len = bytes
        .len()
        .checked_add(usize::from(nul))
        .ok_or(ALLOCATION_FAILED)?;
    let layout = Layout::array::<u8>(len).map_err(|_| ALLOCATION_FAILED)?;
    if len == 0 {
        return Ok(LogifosBuffer::EMPTY);
    }
    let output = allocator(layout);
    if output.is_null() {
        return Err(ALLOCATION_FAILED);
    }
    // SAFETY: The allocator supplies a fresh allocation with the requested
    // layout. The input is live, and the two allocations do not overlap.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len());
        if nul {
            output.add(bytes.len()).write(0);
        }
    }
    Ok(LogifosBuffer { ptr: output, len })
}

fn request(bytes: &[u8], nul: bool) -> Result<LogifosBuffer, u32> {
    let reply = logifos_facade::request_json(bytes).to_string();
    allocate(reply.as_bytes(), nul, |layout| {
        // SAFETY: allocate constructs a nonzero layout before invoking us.
        unsafe { alloc(layout) }
    })
}

fn write_response(
    out: *mut LogifosBuffer,
    work: impl FnOnce() -> Result<LogifosBuffer, u32>,
) -> u32 {
    match contain(work) {
        Ok(Ok(buffer)) => {
            // SAFETY: The public entrypoint establishes a writable out pointer.
            unsafe {
                out.write(buffer);
            }
            OK
        }
        Ok(Err(status)) | Err(status) => status,
    }
}

/// Borrows input and writes an owned length-delimited JSON response.
///
/// OK is transport success, including facade refusals. Other statuses leave
/// `out` null/zero. Null/zero input is the empty message, returning INVALID_JSON
/// in an OK response. Null/positive input and lengths above isize::MAX are
/// transport INVALID_REQUEST. Every OK allocation needs the matching free.
///
/// # Safety
/// `out` must be aligned and writable for one LogifosBuffer, with no live owned
/// response to overwrite, and must not overlap input. Non-null input must point
/// to len initialized, readable bytes in one allocation for this call. Neither
/// input nor out may be concurrently mutated. These obligations apply only to
/// input lengths at most isize::MAX; larger lengths are rejected unread.
#[no_mangle]
pub unsafe extern "C" fn logifos_request_v1(
    input: *const u8,
    len: usize,
    out: *mut LogifosBuffer,
) -> u32 {
    if out.is_null() {
        return INVALID_REQUEST;
    }
    // SAFETY: out is writable by the caller contract. Clear before any failure.
    unsafe {
        out.write(LogifosBuffer::EMPTY);
    }
    if (input.is_null() && len != 0) || len > isize::MAX as usize {
        return INVALID_REQUEST;
    }
    write_response(out, || {
        let bytes = if len == 0 {
            &[]
        } else {
            // SAFETY: Null and impossible lengths were rejected; caller supplies
            // readable input that is borrowed only until this function returns.
            unsafe { std::slice::from_raw_parts(input, len) }
        };
        request(bytes, false)
    })
}

/// Releases one v1 response. A null/zero buffer is a no-op.
///
/// # Safety
/// A nonempty buffer must be the exact unmodified pointer and length returned
/// by this loaded library's logifos_request_v1, still live and freed only once.
/// Do not use this function for static strings or legacy string responses.
#[no_mangle]
pub unsafe extern "C" fn logifos_buffer_free_v1(buffer: LogifosBuffer) {
    if !buffer.ptr.is_null() {
        // SAFETY: Caller transfers the exact allocation and original length.
        // The layout is identical to the one used by allocate.
        unsafe {
            dealloc(buffer.ptr, Layout::from_size_align_unchecked(buffer.len, 1));
        }
    }
}

/// Legacy NUL-terminated adapter, retaining the original two-symbol interface.
/// Prefer logifos_request_v1 to distinguish transport failures. The JSON reply
/// uses the current shared versioned envelope. Null input always returns null.
///
/// # Safety
/// Non-null input must reference len readable initialized bytes in one allocation
/// for this call when len is at most isize::MAX, without concurrent mutation.
#[no_mangle]
pub unsafe extern "C" fn logifos_request_json(input: *const u8, len: usize) -> *mut c_char {
    if input.is_null() || len > isize::MAX as usize {
        return ptr::null_mut();
    }
    contain(|| {
        // SAFETY: The pointer/length preconditions are guaranteed by the caller.
        let bytes = unsafe { std::slice::from_raw_parts(input, len) };
        request(bytes, true).map_or(ptr::null_mut(), |buffer| buffer.ptr.cast())
    })
    .unwrap_or(ptr::null_mut())
}

/// Releases a legacy NUL-terminated response; null is a no-op.
///
/// # Safety
/// reply must be an unmodified, live result from this loaded library's
/// logifos_request_json, freed exactly once. Never pass a v1 buffer or a query.
#[no_mangle]
pub unsafe extern "C" fn logifos_string_free(reply: *mut c_char) {
    if !reply.is_null() {
        // SAFETY: The legacy allocation is unchanged, including its terminator.
        unsafe {
            let len = CStr::from_ptr(reply).to_bytes_with_nul().len();
            dealloc(reply.cast(), Layout::from_size_align_unchecked(len, 1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const NEGOTIATE: &[u8] = br#"{"abi_version":1,"schema_version":1,"op":"negotiate"}"#;

    #[test]
    fn independently_owned_replies_outlive_input() {
        let input = NEGOTIATE.to_vec();
        let mut replies = [LogifosBuffer::EMPTY; 2];
        for reply in &mut replies {
            // SAFETY: Valid input and distinct writable output slots.
            assert_eq!(
                unsafe { logifos_request_v1(input.as_ptr(), input.len(), reply) },
                OK
            );
        }
        drop(input);
        assert_ne!(replies[0].ptr, replies[1].ptr);
        for reply in replies {
            // SAFETY: Each returned buffer is live and released exactly once.
            unsafe {
                let bytes = std::slice::from_raw_parts(reply.ptr, reply.len);
                assert!(!bytes.contains(&0));
                assert!(std::str::from_utf8(bytes).unwrap().contains("capabilities"));
                logifos_buffer_free_v1(reply);
            }
        }
    }

    #[test]
    fn invalid_transport_and_empty_json_are_distinct() {
        let mut reply = LogifosBuffer::EMPTY;
        // SAFETY: Invalid arguments are rejected before reading input; output is valid.
        unsafe {
            assert_eq!(
                logifos_request_v1(ptr::null(), 1, &mut reply),
                INVALID_REQUEST
            );
            assert!(reply.ptr.is_null() && reply.len == 0);
            assert_eq!(
                logifos_request_v1(NEGOTIATE.as_ptr(), usize::MAX, &mut reply),
                INVALID_REQUEST
            );
            assert_eq!(
                logifos_request_v1(ptr::null(), 0, ptr::null_mut()),
                INVALID_REQUEST
            );
            assert_eq!(logifos_request_v1(ptr::null(), 0, &mut reply), OK);
            let bytes = std::slice::from_raw_parts(reply.ptr, reply.len);
            assert!(std::str::from_utf8(bytes).unwrap().contains("INVALID_JSON"));
            logifos_buffer_free_v1(reply);
            logifos_buffer_free_v1(LogifosBuffer::EMPTY);
            logifos_string_free(ptr::null_mut());
        }
    }

    #[test]
    fn allocation_failure_returns_no_buffer_and_allows_reuse() {
        let mut reply = LogifosBuffer::EMPTY;
        let status = write_response(&mut reply, || allocate(b"{}", false, |_| ptr::null_mut()));
        assert_eq!(status, ALLOCATION_FAILED);
        assert!(reply.ptr.is_null() && reply.len == 0);
        // SAFETY: Valid borrowed input and writable output slot after failure.
        unsafe {
            assert_eq!(
                logifos_request_v1(NEGOTIATE.as_ptr(), NEGOTIATE.len(), &mut reply),
                OK
            );
            logifos_buffer_free_v1(reply);
        }
    }

    #[test]
    fn panic_and_panicking_payload_drop_cannot_escape() {
        struct PanickingDrop;
        impl Drop for PanickingDrop {
            fn drop(&mut self) {
                panic!("payload drop");
            }
        }
        let mut reply = LogifosBuffer::EMPTY;
        assert_eq!(
            write_response(&mut reply, || panic!("injected failure")),
            INTERNAL_ERROR
        );
        assert_eq!(
            write_response(&mut reply, || std::panic::panic_any(PanickingDrop)),
            INTERNAL_ERROR
        );
        assert!(reply.ptr.is_null() && reply.len == 0);
        // SAFETY: The same transport remains usable following contained panics.
        unsafe {
            assert_eq!(
                logifos_request_v1(NEGOTIATE.as_ptr(), NEGOTIATE.len(), &mut reply),
                OK
            );
            logifos_buffer_free_v1(reply);
        }
    }
}
