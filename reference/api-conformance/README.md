# Search Kernel IR conformance suite v0.5

Executable, backend-free specification of the **Portable API → Search Kernel IR** boundary.

**v0.5 contains 204 active cases across 13 categories.**

The naming grammar is final:

```text
FlatBuffers API  -- *_to_ir -->  Search Kernel IR  -- lower_* --> backend/Vespa
```

This suite tests:

- stable defaults, schema validation, field capabilities and analyzer identity;
- typed values, documents, writes and patches;
- queries, filters, sorting, facets and ranking;
- projection, highlighting, language and request defaults;
- FlatBuffers presence/null/union normalization at the wire→IR boundary;
- stable machine-readable portable errors;
- Describe/InspectQuery behavior that does not require a backend.

This suite deliberately does **not** freeze bulk execution, browse, grouping, presets, synonyms, curations, lifecycle/management state, analyzer execution, or backend result mapping. Those v0.4 cases are classified in `migration/v0.4-classification.json`.

Run:

```sh
./verify-fixtures.sh
./verify-adapter.sh /path/to/conformance-adapter
```

Goldens are semantic truth. Do not regenerate them from the implementation under test.
