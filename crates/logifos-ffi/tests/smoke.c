#include "logifos.h"

#include <stdio.h>
#include <string.h>

#define CHECK(condition)                                                        \
    do {                                                                        \
        if (!(condition)) {                                                     \
            fprintf(stderr, "check failed at line %d: %s\n", __LINE__,          \
                    #condition);                                                \
            return 1;                                                           \
        }                                                                       \
    } while (0)

int main(void) {
    uint8_t input[] = {'{', '"', 'o', 'p', '"', ':', '"', 'r', 'e',
                       'n', 'd', 'e', 'r', '"', '}'};
    char *first = logifos_request_json(input, sizeof(input));
    char *second = logifos_request_json(input, sizeof(input));
    CHECK(first != NULL && second != NULL && first != second);
    memset(input, ' ', sizeof(input));
    CHECK(strstr(first, "\"ok\":false") != NULL);
    CHECK(strstr(first, "renderer is not implemented") != NULL);
    logifos_string_free(first);
    CHECK(strstr(second, "renderer is not implemented") != NULL);
    logifos_string_free(second);

    CHECK(logifos_request_json(NULL, 0) == NULL);
    CHECK(logifos_request_json(NULL, 1) == NULL);
    CHECK(logifos_request_json(input, SIZE_MAX) == NULL);
    logifos_string_free(NULL);

    const uint8_t render[] = "{\"op\":\"render\"}";
    for (int i = 0; i < 1000; ++i) {
        char *reply = logifos_request_json(render, sizeof(render) - 1);
        CHECK(reply != NULL);
        CHECK(strstr(reply, "\"ok\":false") != NULL);
        CHECK(strstr(reply, "renderer is not implemented") != NULL);
        logifos_string_free(reply);
    }

    const uint8_t invalid[] = "{";
    char *error = logifos_request_json(invalid, sizeof(invalid) - 1);
    CHECK(error != NULL && strstr(error, "\"ok\":false") != NULL);
    logifos_string_free(error);

    puts("C ABI: owned replies, explicit render errors, invalid input, 1000 calls OK");
    return 0;
}
