#pragma once

#include <cstdint>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace searchkernel::native_highlighting {

struct Span {
    uint32_t start_byte;
    uint32_t end_byte;
};

struct Fragment {
    std::string text;
    std::vector<Span> matches;
};

inline bool is_utf8_continuation(uint8_t byte) noexcept {
    return (byte & 0xc0U) == 0x80U;
}

inline bool is_valid_utf8(std::string_view value) noexcept {
    for (size_t index = 0; index < value.size();) {
        const auto byte = static_cast<uint8_t>(value[index]);
        if (byte <= 0x7fU) {
            ++index;
        } else if (byte >= 0xc2U && byte <= 0xdfU) {
            if (index + 1 >= value.size() ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 1]))) {
                return false;
            }
            index += 2;
        } else if (byte == 0xe0U) {
            if (index + 2 >= value.size() ||
                static_cast<uint8_t>(value[index + 1]) < 0xa0U ||
                static_cast<uint8_t>(value[index + 1]) > 0xbfU ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 2]))) {
                return false;
            }
            index += 3;
        } else if ((byte >= 0xe1U && byte <= 0xecU) ||
                   (byte >= 0xeeU && byte <= 0xefU)) {
            if (index + 2 >= value.size() ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 1])) ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 2]))) {
                return false;
            }
            index += 3;
        } else if (byte == 0xedU) {
            if (index + 2 >= value.size() ||
                static_cast<uint8_t>(value[index + 1]) < 0x80U ||
                static_cast<uint8_t>(value[index + 1]) > 0x9fU ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 2]))) {
                return false;
            }
            index += 3;
        } else if (byte == 0xf0U) {
            if (index + 3 >= value.size() ||
                static_cast<uint8_t>(value[index + 1]) < 0x90U ||
                static_cast<uint8_t>(value[index + 1]) > 0xbfU ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 2])) ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 3]))) {
                return false;
            }
            index += 4;
        } else if (byte >= 0xf1U && byte <= 0xf3U) {
            if (index + 3 >= value.size() ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 1])) ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 2])) ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 3]))) {
                return false;
            }
            index += 4;
        } else if (byte == 0xf4U) {
            if (index + 3 >= value.size() ||
                static_cast<uint8_t>(value[index + 1]) < 0x80U ||
                static_cast<uint8_t>(value[index + 1]) > 0x8fU ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 2])) ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 3]))) {
                return false;
            }
            index += 4;
        } else {
            return false;
        }
    }
    return true;
}

inline bool is_utf8_boundary(std::string_view value, size_t offset) noexcept {
    return offset <= value.size() &&
           (offset == 0 || offset == value.size() ||
            !is_utf8_continuation(static_cast<uint8_t>(value[offset])));
}

inline void validate_fragment(const Fragment &fragment) {
    if (!is_valid_utf8(fragment.text)) {
        throw std::runtime_error("Vespa teaser contains invalid UTF-8");
    }
    uint32_t previous_end = 0;
    for (const auto &match : fragment.matches) {
        if (match.start_byte > match.end_byte ||
            match.end_byte > fragment.text.size() ||
            match.start_byte < previous_end ||
            !is_utf8_boundary(fragment.text, match.start_byte) ||
            !is_utf8_boundary(fragment.text, match.end_byte)) {
            throw std::runtime_error("Vespa teaser contains an invalid highlight span");
        }
        previous_end = match.end_byte;
    }
}

// Vespa's embedded docsum configuration uses 0x1f for both highlight markers
// and 0x1e for continuation boundaries. Linguistics annotations use
// U+FFF9/U+FFFA/U+FFFB. Decode those backend-owned delimiters here; do not
// search the returned source text for query terms in Search Kernel.
inline std::vector<Fragment> decode(std::string_view teaser) {
    std::vector<Fragment> fragments;
    if (teaser.empty()) return fragments;
    constexpr std::string_view annotation_anchor = "\xEF\xBF\xB9";
    constexpr std::string_view annotation_separator = "\xEF\xBF\xBA";
    constexpr std::string_view annotation_terminator = "\xEF\xBF\xBB";

    Fragment fragment;
    bool highlighted = false;
    uint32_t highlight_start = 0;
    const auto flush = [&] {
        if (!fragment.text.empty() || !fragment.matches.empty()) {
            validate_fragment(fragment);
            fragments.push_back(std::move(fragment));
            fragment = Fragment{};
        }
    };
    for (size_t index = 0; index < teaser.size();) {
        if (teaser[index] == '\x1e') {
            if (highlighted) {
                throw std::runtime_error("Vespa teaser contains a continuation inside a highlight");
            }
            flush();
            ++index;
        } else if (teaser[index] == '\x1f') {
            if (!highlighted) {
                highlighted = true;
                highlight_start = static_cast<uint32_t>(fragment.text.size());
            } else {
                fragment.matches.push_back({highlight_start, static_cast<uint32_t>(fragment.text.size())});
                highlighted = false;
            }
            ++index;
        } else if (teaser.substr(index, annotation_anchor.size()) == annotation_anchor) {
            const auto separator = teaser.find(annotation_separator, index + annotation_anchor.size());
            const auto terminator = separator == std::string_view::npos
                                        ? std::string_view::npos
                                        : teaser.find(annotation_terminator, separator + annotation_separator.size());
            if (separator == std::string_view::npos || terminator == std::string_view::npos) {
                throw std::runtime_error("Vespa teaser contains malformed interlinear annotation");
            }
            fragment.text.append(teaser.substr(index + annotation_anchor.size(),
                                               separator - index - annotation_anchor.size()));
            index = terminator + annotation_terminator.size();
        } else {
            fragment.text.push_back(teaser[index++]);
        }
    }
    if (highlighted) throw std::runtime_error("Vespa teaser contains unterminated highlight marker");
    flush();
    return fragments;
}

} // namespace searchkernel::native_highlighting
