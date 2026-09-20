#include "searchkernel_vespa_test.h"

#include <cassert>
#include <cstdint>
#include <cstring>
#include <string>
#include <vector>

namespace {

constexpr uint32_t ERROR_CAPACITY = SK_ERROR_MESSAGE_CAPACITY;

struct ErrorBuffer {
    uint8_t bytes[ERROR_CAPACITY]{};

    uint8_t *data() { return bytes; }
};

void expect_ok(int32_t status, const char *operation) {
    assert(status == SK_NATIVE_STATUS_OK && operation);
}

void expect_invalid(int32_t status, const char *operation) {
    assert(status == SK_NATIVE_STATUS_INVALID_ARGUMENT && operation);
}

void verify(uint32_t flags) {
    const char *messages[] = {"first issue", "second issue", "third issue"};
    ErrorBuffer error;
    sk_generic_search_result *result = nullptr;
    expect_ok(sk_test_search_reply_metadata(flags, messages, 3, &result, error.data(),
                                            ERROR_CAPACITY),
              "SearchReply metadata construction");
    assert(result != nullptr);
    assert(sk_generic_search_result_coverage_active(result) == 17);
    assert(sk_generic_search_result_coverage_covered(result) == 11);
    assert(sk_generic_search_result_coverage_target_active(result) == 19);
    assert(sk_generic_search_result_degradation_flags(result) == flags);
    assert(sk_generic_search_result_issue_count(result) == 3);

    const std::vector<std::string> expected = {
        "first issue", "second issue", "third issue",
    };
    for (uint32_t index = 0; index < expected.size(); ++index) {
        const char *message = nullptr;
        uint32_t length = 0;
        expect_ok(sk_generic_search_result_issue_message(
                      result, index, &message, &length, error.data(), ERROR_CAPACITY),
                  "SearchReply issue message");
        assert(message != nullptr);
        assert(std::string(message, length) == expected[index]);
    }

    const char *message = reinterpret_cast<const char *>(1);
    uint32_t length = 99;
    expect_invalid(sk_generic_search_result_issue_message(
                       result, 3, &message, &length, error.data(), ERROR_CAPACITY),
                   "SearchReply issue bounds");
    assert(message == nullptr);
    assert(length == 0);
    assert(error.bytes[0] != 0);
    sk_generic_search_result_destroy(result);
}

} // namespace

int main() {
    verify(SK_DEGRADATION_MATCH_PHASE);
    verify(SK_DEGRADATION_TIMEOUT);
    verify(SK_DEGRADATION_ANN_TIMEOUT);
    verify(SK_DEGRADATION_MATCH_PHASE | SK_DEGRADATION_TIMEOUT |
           SK_DEGRADATION_ANN_TIMEOUT);
}
