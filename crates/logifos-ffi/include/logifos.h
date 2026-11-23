#ifndef LOGIFOS_H
#define LOGIFOS_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define LOGIFOS_ABI_VERSION UINT32_C(1)
#define LOGIFOS_SCHEMA_VERSION UINT32_C(1)
#define LOGIFOS_OK UINT32_C(0)
#define LOGIFOS_INVALID_REQUEST UINT32_C(1)
#define LOGIFOS_ALLOCATION_FAILED UINT32_C(2)
#define LOGIFOS_INTERNAL_ERROR UINT32_C(3)
#define LOGIFOS_FEATURE_BUFFER_V1 UINT64_C(1)
#define LOGIFOS_FEATURE_JSON_V1 UINT64_C(2)
#define LOGIFOS_FEATURE_RENDER UINT64_C(4)

typedef struct logifos_buffer {
    uint8_t *ptr;
    size_t len;
} logifos_buffer;

/* Versions are independent of the package version. Unknown feature bits must
 * be ignored. RENDER is currently absent. Queries allocate nothing. */
uint32_t logifos_abi_version(void);
uint32_t logifos_schema_version(void);
uint64_t logifos_features(void);
/* Static NUL-terminated package version; never modify or free this pointer. */
const char *logifos_engine_version(void);

/* Borrows len bytes without retaining or modifying them. Input has no NUL
 * terminator. Non-null input must name len initialized readable bytes in one
 * allocation, unchanged until return. Lengths above PTRDIFF_MAX are rejected
 * unread. NULL/positive length is INVALID_REQUEST; NULL/zero is an empty JSON
 * message and receives a JSON INVALID_JSON refusal with transport status OK.
 *
 * out must be aligned, writable, disjoint from input, and hold no live response.
 * NULL out is INVALID_REQUEST. A valid out is set to {NULL,0} on all transport
 * failures. OK returns independently owned UTF-8 JSON bytes WITHOUT a NUL
 * terminator. OK includes JSON refusals: inspect the envelope's ok field.
 * Non-OK returns no JSON and must not be read as a facade response.
 *
 * Response allocation failure is ALLOCATION_FAILED; caught Rust panics are
 * INTERNAL_ERROR and do not cross C. Rust's internal infallible allocations or
 * process aborts are not recoverable panics. The library requires panic=unwind.
 * Invalid non-null pointers are caller misuse and cannot be checked.
 * Concurrent calls with independent inputs/outputs are allowed. */
uint32_t logifos_request_v1(const uint8_t *input, size_t len, logifos_buffer *out);

/* Release the EXACT returned pointer AND length once, through the same loaded
 * library that allocated it. Never use C free(), change bytes, mix artifacts,
 * free a static/legacy pointer here, or access a freed buffer. {NULL,0} is a
 * no-op. Other invented/mismatched values and repeated frees are caller misuse.
 * Free every response before unloading its library. This takes a value and
 * does not clear the caller's copy; clear that copy after freeing. */
void logifos_buffer_free_v1(logifos_buffer reply);

/* Compatibility adapters: owned NUL-terminated JSON and matching string free.
 * Prefer v1 for length-delimited bytes and distinguishable transport failures.
 * Null input (including zero length), impossible length, allocation failure or
 * panic returns NULL. JSON uses the shared versioned envelope. */
char *logifos_request_json(const uint8_t *input, size_t len);
/* Release an unchanged legacy reply once; NULL is a no-op. Never pass a v1
 * buffer or static query pointer. Keep its allocating library loaded. */
void logifos_string_free(char *reply);

#ifdef __cplusplus
}
#endif
#endif
