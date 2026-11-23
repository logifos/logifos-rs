#ifndef LOGIFOS_H
#define LOGIFOS_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Borrows len readable bytes from one allocation for this call only. When
 * len <= PTRDIFF_MAX, non-null input must reference that many initialized bytes,
 * valid and unchanged until return; input need not end in NUL.
 * Returns an owned NUL-terminated UTF-8 JSON reply, including facade errors.
 * Rendering is not implemented and returns {"ok":false,"error":...}.
 * Null input (even with len == 0), len > PTRDIFF_MAX, or a caught Rust panic
 * returns NULL. Allocation failure may abort the process.
 * Keep the library loaded until every reply has been freed. */
char *logifos_request_json(const uint8_t *input, size_t len);

/* Free an unmodified reply exactly once with this same library; NULL is a no-op.
 * Do not use C free(), modify the reply, or access it after this call. */
void logifos_string_free(char *reply);

#ifdef __cplusplus
}
#endif

#endif
