# WASM transport artifacts

This unpublished MIT crate wraps `logifos-facade`; all JSON decoding belongs to
`logifos-json`. Only version negotiation succeeds. Parse returns
`NOT_IMPLEMENTED`; SVG refuses with `MISSING_FONT` or `UNPINNED_CONTRACT`, and
PNG returns `UNSUPPORTED_OUTPUT`. No engine operation or successful rendering is
advertised. Formula/syntax pins remain unresolved. Native and WASM share these
wire envelopes and errors, independently of their memory-ownership APIs.

## Pinned build

Use Rust **1.95.0**, target `wasm32-unknown-unknown`, the committed `Cargo.lock`,
and **wasm-bindgen 0.2.130** for both the Rust dependency and CLI. Install tools
only into a chosen scratch directory, for example:

```sh
rustup target add --toolchain 1.95.0 wasm32-unknown-unknown
cargo +1.95.0 install wasm-bindgen-cli --version 0.2.130 --locked -j 2 --root "$TMPDIR/tools"
export WASM_BINDGEN="$TMPDIR/tools/bin/wasm-bindgen"
export CARGO_TARGET_DIR="$TMPDIR/target"
export WASM_CONTRACT_DIR=/path/to/authoritative/contracts
bash scripts/build-wasm.sh "$TMPDIR/wasm-candidate"
bash scripts/check-wasm.sh "$TMPDIR/wasm-candidate"
```

The repository stays on `stable`; only the WASM build selects Rust 1.95.0.
The output directory must be new. The build uses the pinned tool versions,
`--locked`, release optimization and a 64 MiB linear-memory maximum. It emits
separate `nodejs` CommonJS and `web` ES module packages, declarations, MIT notice,
dependency MIT/Unicode notices and `receipt.json`. Both packages are private; there is no publication command.
Generation uses bindgen's documented [deployment targets](https://wasm-bindgen.github.io/wasm-bindgen/reference/deployment.html).
The receipt binds exact source files, Cargo.lock, toolchain, the authoritative
request/response/pin files, and every generated glue/WASM/package file. Missing,
extra, altered or mismatched files fail closed. Consumers must trust the receipt
from their artifact supplier; hashes are integrity checks, not signatures.
Artifacts are scratch build results, never committed binaries.

## Ownership and use

Node consumers load the generated package's `index.js` and call
`request_json(new TextEncoder().encode(JSON.stringify(request)))`. The result is
an independent owned `Uint8Array` of UTF-8 JSON. Decode it with `TextDecoder`
and parse the JSON. Inputs must be `Uint8Array`; JavaScript API misuse throws
`TypeError`. Wire errors are JSON, with `abi_version`, `schema_version`, `ok:false`
and structured `error:{code,message,path,stage}`. Each request explicitly uses
integer `abi_version:1`, `schema_version:1`, and an `op`.

For web output, import the default initializer and `request_json` from `index.js`,
await initialization, then call the same bytes API. Serve the glue and its paired
`logifos_bg.wasm` together. There is no filesystem or Node import in this entry.
The Node acceptance checks also initialize the web module from supplied bytes;
that checks the recipe, and does **not** qualify a browser.

Bindgen copies input and frees temporary Rust allocations after each call. It
copies returned bytes into JavaScript and frees their Rust storage. No explicit
free function is needed in JavaScript. Oversized input is refused by the package
entry before bindgen copies the complete caller buffer; the shared Rust facade
produces that refusal from a bounded probe. Do not call raw glue as a public API.
No options, source bytes or font resources survive a call. A successful reply
never implies a renderer is available; inspect `implemented_operations`.

Unexpected panics and allocation exhaustion trap/abort rather than unwind across
WASM. Discard a trapped instance and initialize a fresh one; no JSON allocation
or instance recovery is promised after exhaustion. Ordinary malformed requests
and configured limit refusals return JSON and allow immediate reuse.

## Budget qualification

Initial transport budgets are 256 KiB complete request, 64 KiB UTF-8 source,
32 JSON container levels, 128 KiB decoded individual/total font bytes, 16 font
resources and 16 KiB complete response. Native uses the same ceilings. Callers
may lower limits, never raise them. A response too small to hold an envelope
returns a minimal `RESOURCE_LIMIT` envelope bounded by the hard response ceiling;
no partial success is emitted. Tree/glyph ceilings retain the wire contract's
100,000 nodes/glyphs and tree depth 128; these are not measured renderer budgets
and cannot qualify unimplemented operations. Font tests check supplied byte
identity only, not usable font files, outlines or MATH data.

The named Node checks measure exact request/source/font boundaries, nesting,
malformed UTF-8/JSON, duplicate keys, caller reductions, refusal/recovery, independent
reply ownership, and warmed linear-memory growth over 64 repeated worst-case
transport cycles. On Node 24.22.2, 256 KiB dense JSON arrays brought warmed linear
memory to 7,602,176 bytes, unchanged after 64 reuse cycles. The checks print fresh
memory and Node measurements for each tested candidate; these observations
justify only this initial bounded transport budget.
Rendering and browser resource/performance budgets require their own measurements.
The checker also compares native wire results and rejects tampered receipts.
