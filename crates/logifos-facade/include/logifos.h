/* logifos C ABI v1: owned request/response buffers, no retained state. */
#ifndef LOGIFOS_H
#define LOGIFOS_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Returns a NUL-terminated UTF-8 JSON reply owned by the caller. */
uint8_t *logifos_request_json(const uint8_t *request, size_t request_len,
                              size_t *reply_len);

/* Frees a reply previously returned by logifos_request_json. */
void logifos_reply_free(uint8_t *reply, size_t reply_len);

/* The transport and envelope versions this library speaks. */
uint32_t logifos_abi_version(void);
uint32_t logifos_schema_version(void);

#ifdef __cplusplus
}
#endif
#endif
