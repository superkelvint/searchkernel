# Frozen policy decisions in v0.4

1. `text` fields are searchable, stored, prefix-capable and typo-capable by default. Field capability is distinct from effective request policy.
2. Searchable `keyword` fields use `keyword/v1`, are typo-disabled by default, and prefix/infix are opt-in capabilities.
3. `standard/v1`, `english/v1`, and `keyword/v1` are exact Search Kernel-owned analyzer contracts; see `ANALYZERS.md`.
4. Index definition `search_fields` contains only field membership and weight. Effective prefix/infix/typo policy exists only on a resolved search request.
5. Default search is literal, all-terms, last-term-prefix, standard/v1 typo tolerance, synonyms enabled, limit 20.
6. Resolution precedence is index defaults < mutable settings < named preset < explicit request; then curations; then synonyms. See `RESOLUTION_ORDER.md`.
7. Named presets are looked up from managed state. Unknown names return `unknown_preset`.
8. Curations match the original normalized query key, apply by priority descending then name ascending, query replacement precedes synonym expansion, added filters are ANDed after caller filters, and hide beats pin.
9. Synonym expansion is owned by the high-level core and appears explicitly in `synonym_expansions`; `synonyms=false` suppresses installed synonym state.
10. Infix is a field capability. If an infix-capable field participates in a search, its resolved search-field entry has `infix=true`; direct matches outrank prefix matches, which outrank infix matches under `default/v1`.
11. User syntax is opt-in and v1 parses quoted phrases and unary `-term` exclusion.
12. Page numbering is one-based and cannot be combined with explicit limit/offset. Field sort missing values default to `last`.
13. Value facets require a facetable field. Numeric range facets require a numeric field and do not require value-facet capability. Overlapping numeric ranges are allowed.
14. `insert=create only`, `upsert=full replacement`, `patch=partial mutation`, `delete=idempotent`. A patch may target a field only once.
15. Static bulk validation is all-or-nothing. Runtime failures are per-item. Batch execution is ordered and non-atomic. Bulk request and item-result shapes are canonical.
16. `ne(field,value)` requires field presence. `not(eq(...))` is logical complement and therefore matches missing optional fields. Empty boolean groups and empty `one_of` are invalid.
17. Highlights/snippets use matched/unmatched text segments, not implementation-dependent offsets. Analyze results use one stable token shape without offsets.
18. Ranking and hard sorting are distinct. Deterministic final ordering is by DocumentId.
19. Projection/highlight field lists are ordered sets; duplicate names are removed preserving first occurrence. Snippet configs and hard-sort fields must be unique.
20. `group_by` defaults to three hits per group. `group_by` and `distinct_by` are mutually exclusive.
21. Logical index lifecycle is backend-independent: create-existing fails, open-missing fails, close invalidates that handle, and an index can subsequently be reopened.
22. Management upserts replace the previous value under the same name; deleting an absent management object is idempotent.
23. `DocumentId` is opaque Unicode with the exact validation rules in `DOCUMENT_IDS.md`.
24. Error-detail shapes are stable per error code and are recursively validated.
