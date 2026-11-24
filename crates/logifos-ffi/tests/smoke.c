#include "logifos.h"

#include <stdio.h>
#include <string.h>

#define CHECK(condition) \
    do { if (!(condition)) { \
        fprintf(stderr, "check failed at line %d: %s\n", __LINE__, #condition); \
        return 1; \
    } } while (0)

static int contains(logifos_buffer buffer, const char *needle) {
    size_t len = strlen(needle);
    if (buffer.len < len) return 0;
    for (size_t i = 0; i <= buffer.len - len; ++i) {
        if (memcmp(buffer.ptr + i, needle, len) == 0) return 1;
    }
    return 0;
}

static int expect(const uint8_t *input, size_t len, const char *marker) {
    logifos_buffer reply = {NULL, 0};
    CHECK(logifos_request_v1(input, len, &reply) == LOGIFOS_OK);
    CHECK(reply.ptr != NULL && reply.len > 0);
    CHECK(memchr(reply.ptr, 0, reply.len) == NULL);
    CHECK(contains(reply, "\"abi_version\":1"));
    CHECK(contains(reply, "\"schema_version\":1"));
    CHECK(contains(reply, marker));
    logifos_buffer_free_v1(reply);
    return 0;
}

int main(void) {
    CHECK(logifos_abi_version() == LOGIFOS_ABI_VERSION);
    CHECK(logifos_schema_version() == LOGIFOS_SCHEMA_VERSION);
    CHECK(logifos_features()
          == (LOGIFOS_FEATURE_BUFFER_V1 | LOGIFOS_FEATURE_JSON_V1 | LOGIFOS_FEATURE_RENDER));
    CHECK(logifos_engine_version() != NULL && strlen(logifos_engine_version()) > 0);
    CHECK(logifos_engine_version() == logifos_engine_version());

    uint8_t input[] = "{\"abi_version\":1,\"schema_version\":1,\"op\":\"negotiate\"}";
    logifos_buffer first = {NULL, 0}, second = {NULL, 0};
    CHECK(logifos_request_v1(input, sizeof(input) - 1, &first) == LOGIFOS_OK);
    CHECK(logifos_request_v1(input, sizeof(input) - 1, &second) == LOGIFOS_OK);
    CHECK(first.ptr != NULL && second.ptr != NULL && first.ptr != second.ptr);
    memset(input, ' ', sizeof(input));
    CHECK(contains(first, "\"ok\":true"));
    CHECK(contains(first, "\"implemented_operations\":[\"negotiate\",\"parse\",\"render\"]"));
    CHECK(contains(first, "\"outputs\":[\"svg\"]"));
    logifos_buffer_free_v1(first);
    CHECK(contains(second, "\"kind\":\"capabilities\""));
    logifos_buffer_free_v1(second);

    logifos_buffer reply = {NULL, 99};
    CHECK(logifos_request_v1(NULL, 1, &reply) == LOGIFOS_INVALID_REQUEST);
    CHECK(reply.ptr == NULL && reply.len == 0);
    CHECK(logifos_request_v1(input, SIZE_MAX, &reply) == LOGIFOS_INVALID_REQUEST);
    CHECK(reply.ptr == NULL && reply.len == 0);
    CHECK(logifos_request_v1(NULL, 0, NULL) == LOGIFOS_INVALID_REQUEST);
    CHECK(expect(NULL, 0, "INVALID_JSON") == 0);
    CHECK(expect(input, 0, "INVALID_JSON") == 0);
    logifos_buffer_free_v1(reply);

    const uint8_t invalid_utf8[] = {0xff};
    CHECK(expect(invalid_utf8, sizeof(invalid_utf8), "INVALID_UTF8") == 0);
    CHECK(expect((const uint8_t *)"{", 1, "INVALID_JSON") == 0);
    const char *duplicates = "{\"op\":\"negotiate\",\"op\":\"render\"}";
    CHECK(expect((const uint8_t *)duplicates, strlen(duplicates), "INVALID_JSON") == 0);
    const char *wrong_version = "{\"abi_version\":2,\"schema_version\":1,\"op\":\"negotiate\"}";
    CHECK(expect((const uint8_t *)wrong_version, strlen(wrong_version), "UNSUPPORTED_ABI") == 0);
    const char *parse = "{\"abi_version\":1,\"schema_version\":1,\"op\":\"parse\",\"format\":\"mathml\",\"source\":\"<math><mi>\\u03b1</mi></math>\"}";
    CHECK(expect((const uint8_t *)parse, strlen(parse), "\"model\":{") == 0);
    CHECK(expect((const uint8_t *)parse, strlen(parse), "\"ok\":true") == 0);
    const char *render = "{\"abi_version\":1,\"schema_version\":1,\"op\":\"render\",\"formula\":{},\"formula_contract\":{\"origin\":\"test\",\"revision\":\"0000000000000000000000000000000000000000\",\"schema_path\":\"schema.json\",\"sha256\":\"0000000000000000000000000000000000000000000000000000000000000000\"},\"output\":\"svg\"}";
    for (int i = 0; i < 1000; ++i) {
        CHECK(expect((const uint8_t *)render, strlen(render), "\"ok\":false") == 0);
    }
    CHECK(expect((const uint8_t *)render, strlen(render), "MISSING_FONT") == 0);

    char *legacy = logifos_request_json((const uint8_t *)render, strlen(render));
    CHECK(legacy != NULL && strstr(legacy, "MISSING_FONT") != NULL);
    logifos_string_free(legacy);
    CHECK(logifos_request_json(NULL, 0) == NULL);
    CHECK(logifos_request_json(input, SIZE_MAX) == NULL);
    logifos_string_free(NULL);

    puts("C ABI: versions/features, negotiation, render refusal, ownership, null/invalid input, legacy adapters, 1000 free cycles OK");
    return 0;
}
