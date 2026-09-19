# Changelog

## 0.6.0

- records successful V2 `*_to_ir()` results as deterministic internal IR snapshots;
- observes the complete `SearchRequestIr` and `IndexDefinitionIr` state, including
  grouping, ranking-request properties, inputs, presentation, query controls, and
  schema extensions;
- removes the legacy `SearchRequestIr.ranking` compatibility projection;
- keeps unsupported V2 constructs as negative conversion cases rather than
  manufacturing IR for them;
- adds focused V2 request/schema coverage and serializer sensitivity tests.

## 0.5.0

- renamed API→IR operations from `resolve_*` to `*_to_ir`;
- replaced `Resolved*` model terminology with Search Kernel `*Ir`;
- narrowed active acceptance to the frozen Portable API→IR contract;
- deferred v0.4 browse/bulk/group/preset/synonym/curation/lifecycle/management semantics;
- removed analyzer-execution and result-mapping requirements from the IR corpus;
- added FlatBuffers wrapper-presence, nullable string/vector, optional-scalar and union-NONE cases;
- changed error goldens to machine-stable `ErrorCode` + detail-code/path + retryable projections;
- added structured QueryExpr IR coverage.

## 0.4.0

See `migration/v0.4-classification.json` for disposition of every v0.4 case.
