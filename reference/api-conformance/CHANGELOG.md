# Changelog

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
