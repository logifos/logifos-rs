/* The C consumer smoke: one render request against the v1 ABI. */
#include <stdio.h>
#include <string.h>
#include "logifos.h"

int main(void) {
    const char *request =
        "{\"abi_version\":1,\"schema_version\":1,\"op\":\"negotiate\"}";
    size_t reply_len = 0;
    uint8_t *reply = logifos_request_json(
        (const uint8_t *)request, strlen(request), &reply_len);
    if (!reply) return 1;
    printf("%.*s\n", (int)reply_len, (const char *)reply);
    int ok = strstr((const char *)reply, "\"ok\":true") != NULL;
    logifos_reply_free(reply, reply_len);
    return ok ? 0 : 1;
}
