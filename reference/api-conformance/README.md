# Search Kernel high-level conformance suite v0.4

Executable, backend-free specification for the high-level Search Kernel semantics.

**v0.4 contains 260 cases across 18 categories.**

This suite does **not** test Vespa matching or persistence. It tests:

- schema/default normalization;
- validation and stable portable error payloads;
- resolved search/write/facet/ranking requests;
- exact Search Kernel analyzer semantics;
- portable response mapping;
- managed presets, search settings, synonyms and curations applied to searches;
- field capability versus effective query policy;
- cross-language value and DocumentId portability.

Run:

```sh
./verify-fixtures.sh
./verify-adapter.sh /path/to/conformance-adapter
```

An implementation is conformant only when it passes the canonical suite without modifying `cases/`, `golden/`, `fixtures/`, `model/`, the semantic docs, or the error registry.
