# Changelog

## 0.4.0

- fixed canonical bulk request and bulk result shapes;
- fixed canonical analyze result/token shape;
- registered and recursively validates nested errors;
- stabilized error-detail schemas;
- separated field capability from effective request policy;
- froze exact analyzer semantics;
- added named-preset, mutable-setting, synonym, curation and infix composition coverage;
- froze document ID domain;
- added deterministic resolution-order contract.


## v0.3.0

Freeze-readiness expansion over v0.2.0.

- expanded from 197 to 240 carefully chosen cases across 17 categories;
- added backend-free logical index lifecycle semantics;
- added request-level duplicate detection and ordered-set canonicalization;
- completed hard-sort missing-value and deterministic tie-break coverage;
- added facet ordering/default-limit and grouping default/conflict coverage;
- added dynamic-document, Unicode highlight/analyze, successful bulk, and empty-group response mapping;
- added management replacement/idempotent-delete semantics;
- strengthened semantic self-linting for duplicate-free resolved sets and deterministic sorts.

## v0.2.0

Correctness release over v0.1.0.

- fixed copied query text in successful search goldens;
- made text prefix/typo capabilities consistent with default search policy;
- made resolved search-field shape canonical;
- made autocomplete preset schema-valid;
- completed the error registry;
- removed Python bytecode from immutable state;
- strengthened fixture validation with semantic invariants and exact `cases.json` checking;
- froze null/missing/empty value behavior and portable i64/f64 rules;
- added analyze request/result conformance;
- added pagination conflicts, custom typo policy, per-field search policy, user-syntax AST, and stored-only errors;
- added filter edge semantics and set canonicalization;
- added numeric/text facet edge semantics;
- added write/get/delete response mapping and stricter patch/bulk semantics;
- replaced encoding-dependent highlight offsets with portable text segments;
- added grouping/distinct and grouped response cases;
- added backend-free management-state scenarios for presets, synonyms, curations, and search settings;
- upgraded adapter protocol to `sk-conformance/v2`.

## v0.1.0

Initial standalone high-level conformance suite.
