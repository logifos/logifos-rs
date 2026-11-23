# Portable native C ABI

`logifos-ffi` is an unpublished `cdylib` packaging crate. Native exports and
allocation ownership live here; all JSON request behavior lives in
`logifos-facade`, and decoding goes through `logifos-json`.

[include/logifos.h](include/logifos.h) declares ABI 1. Query
`logifos_abi_version`, `logifos_schema_version`, `logifos_engine_version` and
`logifos_features` before calling the transport. Package, ABI and schema
versions are independent. Feature bits currently advertise only buffer transport
and versioned JSON negotiation/refusals. Ignore unknown feature bits.

`logifos_request_v1(input, len, out)` borrows input without changing or retaining
it. Input and output are length-delimited UTF-8 JSON, without a BOM or trailing
NUL. The caller retains input ownership. `out` must be aligned, writable,
disjoint from input, and hold no live response. A non-null input must reference
`len` initialized readable bytes in one allocation, valid until return.

A transport `LOGIFOS_OK` returns an independent `logifos_buffer`, even for
`ok:false` JSON refusals. Its lifetime is independent of the input. Release the
exact pointer and length once with `logifos_buffer_free_v1` from the same loaded
library. Never modify it, call C `free`, mix artifacts, or unload its library
before freeing all responses. Free takes the buffer by value; clear the caller's
copy afterwards. A null/zero buffer is a no-op. Invalid pointers, invented
lengths and repeated frees violate the caller contract and cannot be recovered.

Null/positive input, lengths above `PTRDIFF_MAX`, and null output are transport
`LOGIFOS_INVALID_REQUEST`. Null/zero input is an empty message: transport succeeds
and the facade returns `INVALID_JSON`. Every transport failure leaves a valid
output slot null/zero. A failed response allocation is `LOGIFOS_ALLOCATION_FAILED`;
a contained panic is `LOGIFOS_INTERNAL_ERROR`. Neither promises a JSON response.
Panics use `catch_unwind`; panic payloads are deliberately retained to prevent a
panicking destructor from escaping. Builds with `panic=abort` are rejected.
The response allocator is fallible, but Rust/serde intermediate allocations
still use the standard infallible allocator: process-wide OOM or explicit abort
cannot be recovered by panic containment.

Only this contract exchange succeeds:

```json
{"abi_version":1,"schema_version":1,"op":"negotiate"}
```

Its capabilities report empty implemented operations and outputs and null
external contract pins. Parse, transform and render refuse; no SVG, metrics or
successful render placeholder is generated. The facade validates versions,
closed envelopes, basic options and per-call byte/depth/source ceilings. Font
resource validation and external payload qualification remain unimplemented:
nonempty resources receive `NOT_IMPLEMENTED`, including on negotiation. Valid
core PNG requests receive `UNSUPPORTED_OUTPUT`. These checks qualify transport
and refusals, not full font/model validation or rendering. An impossibly small
response limit receives a bounded `RESOURCE_LIMIT` diagnostic, whose envelope
may itself exceed that requested limit.

The existing `logifos_request_json` and `logifos_string_free` remain as legacy
NUL-terminated adapters. Their JSON uses the shared versioned envelope. They
return null for transport failure and cannot distinguish its cause. Never pass
legacy responses to buffer free, buffer responses to string free, or static
version query strings to either free.

Build a host candidate with `cargo build -p logifos-ffi --locked -j 2`, or run
`bash scripts/check-ffi.sh`. The script builds the debug cdylib, inspects its
exports and queries, writes a local artifact receipt alongside it, rejects ten
altered receipts/artifacts, then compiles and runs the C consumer and, when
available, the same consumer as C++. Cases cover negotiation, typed refusals,
input independence, null/invalid inputs, legacy adapters and 1000 free cycles.
Rust tests inject response allocation failure and panics through the production
containment helpers; no test-only export or request operation is added.

The receipt binds artifact bytes, header and build-input hashes, ABI/schema and
package versions, feature bits, host target, compiler and Rust flags. It is a
local consistency receipt, not a signature or a published platform qualification.
`CARGO_TARGET_DIR` selects the artifact directory; `CC`, `CXX` and `TMPDIR`
select the compilers and temporary executable directory. Linux `.so` is the
locally exercised target; macOS `.dylib` has a recipe but needs execution there.
Other targets are unqualified. The crate remains unpublished.

To inspect a matching candidate without rebuilding, use:

```sh
bash scripts/check-ffi.sh --prebuilt /absolute/path/to/logifos-ffi.receipt.json
```

The library must be beside its receipt. Verification checks artifact and source
hashes before loading it, then checks exports, versions and features before the
consumer executes. Missing/stale receipts or changed headers, sources, flags,
toolchains and library bytes fail. No download, publication or remote action is
performed. Receipts and binaries stay outside tracked source paths.
