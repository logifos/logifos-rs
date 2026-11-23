# Native C ABI scaffold

`logifos-ffi` is an unpublished `cdylib` transport for `logifos-facade`.
Rendering is not implemented. A request such as `{"op":"render"}` returns
`{"error":"renderer is not implemented","ok":false}`. Request handling and
errors belong to the facade; this crate only borrows input, serializes replies,
and manages native allocations.

The two exports are declared in [include/logifos.h](include/logifos.h):

- `logifos_request_json(input, len)` borrows exactly `len` bytes for the call,
  with no terminating NUL required. A non-null input must point to initialized,
  readable bytes in one allocation and remain unchanged until return. A reply
  is independently owned, NUL-terminated UTF-8 JSON and outlives the input.
- `logifos_string_free(reply)` releases a reply exactly once using the same
  loaded library. Do not modify replies, use C `free`, or access freed replies.
  Free all replies before unloading the library. Freeing null does nothing.

Null input (including zero length), lengths above `isize::MAX`, or a caught
Rust panic return null. Non-null zero-length input is an empty JSON request and
receives a facade error. Invalid pointers cannot be checked by this API; the
caller must meet the header's contract. Allocation failure may abort.

From the workspace root, build with `cargo build -p logifos-ffi --locked -j 2`.
The debug library is `target/debug/liblogifos_ffi.so` on Linux or
`target/debug/liblogifos_ffi.dylib` on macOS, under `CARGO_TARGET_DIR` when set.
On a Unix host with a C compiler, run `bash scripts/check-ffi.sh` to build the
library, compile and link a C consumer against the header and shared library,
and exercise ownership, explicit errors, null handling, and repeated calls.
`CC` selects the compiler and `TMPDIR` selects the temporary executable directory.

The full native export set is later work. No runtime bindings are provided here.
