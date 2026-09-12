# Search resolution order

For a high-level search request, resolution is deterministic:

1. Start with immutable index search defaults.
2. Overlay mutable search settings.
3. Overlay the named preset, if any.
4. Overlay explicit request options.
5. Preserve the caller text as `query.original_text` and normalize/parse it according to the selected syntax.
6. Match curations against a query key formed by NFKC + Unicode case folding + trimming and collapsing whitespace. Matching curations are ordered by priority descending, then name ascending.
7. Apply the highest-priority query replacement, if any, producing `query.text`.
8. Analyze the effective query and resolve synonyms. Direct query terms remain primary; synonym alternatives are explicit in `synonym_expansions` and rank below direct matches under `default/v1`.
9. AND curation-added filters after the caller filter, in curation application order.
10. Resolve pins/hides. Hide wins over pin for the same document.
11. Resolve effective per-field prefix, infix and typo behavior by intersecting request policy with field capability.

Explicit request options always beat mutable settings and presets. Curations do not change schema/index-time semantics.
