#pragma once

#include <cstdint>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
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

// Vespa's embedded docsum configuration uses control-byte highlight markers
// and preserves linguistics annotations using U+FFF9/U+FFFA/U+FFFB. Decode
// those backend-owned delimiters here; do not search the returned source text
// for query terms in Search Kernel.
inline std::optional<Fragment> decode(std::string_view teaser) {
    if (teaser.empty()) return std::nullopt;
    constexpr std::string_view annotation_anchor = "\xEF\xBF\xB9";
    constexpr std::string_view annotation_separator = "\xEF\xBF\xBA";
    constexpr std::string_view annotation_terminator = "\xEF\xBF\xBB";

    Fragment fragment;
    bool highlighted = false;
    uint32_t highlight_start = 0;
    for (size_t index = 0; index < teaser.size();) {
        if (teaser.substr(index, 3) == "<b>") {
            if (highlighted) throw std::runtime_error("Vespa teaser contains nested highlight markup");
            highlighted = true;
            highlight_start = static_cast<uint32_t>(fragment.text.size());
            index += 3;
        } else if (teaser.substr(index, 4) == "</b>") {
            if (!highlighted) throw std::runtime_error("Vespa teaser contains unmatched highlight markup");
            fragment.matches.push_back({highlight_start, static_cast<uint32_t>(fragment.text.size())});
            highlighted = false;
            index += 4;
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
    if (highlighted) throw std::runtime_error("Vespa teaser contains unterminated highlight markup");
    return fragment;
}

} // namespace searchkernel::native_highlighting
