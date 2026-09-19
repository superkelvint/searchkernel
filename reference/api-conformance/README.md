# Search Kernel IR conformance suite v0.6

Executable, backend-free specification of the **Portable API → Search Kernel IR** boundary.

**v0.6 contains the active API→IR cases and records an internal V2 IR snapshot.**

Successful outputs are deterministic observations of owned `*_to_ir()` values.
They are not public JSON, portable client DTOs, or backend result formats.
Optional IR members are represented as `null`, empty collections remain empty,
and ordered IR collections preserve their order. Unsupported V2 wire constructs
remain negative conversion cases with stable error projections.

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
