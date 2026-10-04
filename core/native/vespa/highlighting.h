#pragma once

#include <cstdint>
#include <optional>
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

// Vespa's embedded docsum configuration uses 0x1f highlight markers and 0x1e
// continuation markers between disjoint teaser regions. It preserves
// linguistics annotations using U+FFF9/U+FFFA/U+FFFB. Decode those backend-
// owned delimiters here; do not search the returned source text for query
// terms in Search Kernel.
inline std::optional<std::vector<Fragment>> decode(std::string_view teaser) {
    if (teaser.empty()) return std::nullopt;
    constexpr std::string_view annotation_anchor = "\xEF\xBF\xB9";
    constexpr std::string_view annotation_separator = "\xEF\xBF\xBA";
    constexpr std::string_view annotation_terminator = "\xEF\xBF\xBB";

    std::vector<Fragment> fragments;
    Fragment fragment;
    bool highlighted = false;
    uint32_t highlight_start = 0;
    bool continued_start = false;
    const auto flush_fragment = [&]() {
        if (!fragment.text.empty() || !fragment.matches.empty()) {
            fragments.push_back(std::move(fragment));
            fragment = Fragment{};
        }
    };
    for (size_t index = 0; index < teaser.size();) {
        if (continued_start) {
            continued_start = false;
            const auto terminator = teaser.find(annotation_terminator, index);
            const auto anchor = teaser.find(annotation_anchor, index);
            const auto separator = teaser.find(annotation_separator, index);
            const auto boundary = teaser.find_first_of("\x1e\x1f", index);
            // SummaryDesc can begin a continued excerpt inside annotation
            // metadata. Only its suffix before the first backend delimiter
            // belongs to the clipped annotation, not to visible surface text.
            if (terminator != std::string_view::npos && terminator < anchor &&
                terminator < separator && terminator < boundary) {
                index = terminator + annotation_terminator.size();
                continue;
            }
        }
        if (teaser[index] == '\x1f') {
            if (!highlighted) {
                highlighted = true;
                highlight_start = static_cast<uint32_t>(fragment.text.size());
            } else {
                fragment.matches.push_back({highlight_start, static_cast<uint32_t>(fragment.text.size())});
                highlighted = false;
            }
            ++index;
        } else if (teaser[index] == '\x1e') {
            if (highlighted) {
                throw std::runtime_error("Vespa teaser continuation interrupts highlight markup");
            }
            flush_fragment();
            continued_start = true;
            ++index;
        } else if (teaser.substr(index, annotation_anchor.size()) == annotation_anchor) {
            const auto separator = teaser.find(annotation_separator, index + annotation_anchor.size());
            const auto continuation = teaser.find('\x1e', index + annotation_anchor.size());
            const auto terminator = separator == std::string_view::npos
                                        ? std::string_view::npos
                                        : teaser.find(annotation_terminator, separator + annotation_separator.size());
            // Pinned Juniper SummaryDesc::get_summary can end an excerpt in
            // annotation metadata and append its continuation delimiter before
            // the terminator. Preserve the complete surface form before the
            // separator; discard only that clipped metadata. Without the
            // backend continuation boundary a missing terminator is malformed.
            const bool clipped_metadata = continuation != std::string_view::npos &&
                                          (terminator == std::string_view::npos || continuation < terminator);
            // A continued excerpt may also end before the annotation's
            // separator. These backend-selected bytes are surface text;
            // preserve them without manufacturing a highlight or completion.
            if (continuation != std::string_view::npos &&
                (separator == std::string_view::npos || continuation < separator)) {
                if (highlighted) throw std::runtime_error("Vespa teaser contains malformed interlinear annotation");
                fragment.text.append(teaser.substr(index + annotation_anchor.size(),
                                                   continuation - index - annotation_anchor.size()));
                index = continuation;
                continue;
            }
            if (separator == std::string_view::npos ||
                (clipped_metadata && highlighted) ||
                (terminator == std::string_view::npos && !clipped_metadata)) {
                throw std::runtime_error("Vespa teaser contains malformed interlinear annotation");
            }
            fragment.text.append(teaser.substr(index + annotation_anchor.size(),
                                               separator - index - annotation_anchor.size()));
            index = clipped_metadata ? continuation : terminator + annotation_terminator.size();
        } else {
            fragment.text.push_back(teaser[index++]);
        }
    }
    if (highlighted) throw std::runtime_error("Vespa teaser contains unterminated highlight markup");
    flush_fragment();
    if (fragments.empty()) return std::nullopt;
    return fragments;
}

} // namespace searchkernel::native_highlighting
