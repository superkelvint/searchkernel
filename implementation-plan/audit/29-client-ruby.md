# 29. Ruby client — `clients/ruby`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)

**Minimum evidence to PASS this section:** `SRC + UNIT + GEN + PKG`; each advertised Ruby capability requires the corresponding production HTTP/FlatBuffers-JSON `E2E` evidence, and managed-local lifecycle claims additionally require `HARD` evidence for ownership/race behavior.

**Shared closure gates:** apply `SK-20-001..SK-20-013` where applicable, `SK-21-013`, `SK-22-002`, `SK-22-004`, `SK-23-016`, `SK-23-019`, `SK-26-001`, `SK-26-004`, `SK-26-007`, `SK-27-001`, `SK-27-004`, `SK-27-006`, `SK-27-007`, and `SK-27-009`; this module supplies Ruby-specific source and evidence mapping without duplicating those obligations.

## 29.1 Canonical representation and values

- [ ] **SK-29-001** Ruby request/response objects represent canonical FlatBuffers JSON table bodies and union discriminators; no independent domain DTO or operation-specific HTTP route exists.
- [ ] **SK-29-002** `emit_bfbs_contract.rb` and `consume_bfbs_response.rb` exercise Ruby-produced requests and Ruby-consumed responses through the official BFBS parser/printer.
- [ ] **SK-29-003** Empty hashes remain objects, empty arrays remain lists, and absent, null, false, and zero values retain distinct schema meanings through Ruby JSON encoding/decoding.
- [ ] **SK-29-004** Signed/unsigned integer boundaries, bytes, nested values, tensors, and non-finite portable values preserve canonical union types without narrowing or content-based type inference.
- [ ] **SK-29-005** Result/value decoders preserve recursive ordering and reject unknown variants, malformed fields, duplicate keys, and invalid response roots.
- [ ] **SK-29-006** Protocol version, operation unions, lifecycle variants, and structured portable errors remain synchronized with the canonical FlatBuffers schema.

## 29.2 Transport, semantics, and builders

- [ ] **SK-29-007** HTTP transport sends standard FlatBuffers JSON only to the generic execute endpoint and does not parse or resolve Search Kernel domain semantics.
- [ ] **SK-29-008** URL construction, TLS, timeouts, request/response size limits, redirect policy, HTTP errors, malformed JSON, and closed-transport behavior fail deterministically.
- [ ] **SK-29-009** Target strings are represented without a Ruby-owned copy of canonical target grammar; invalid domain values reach server/IR validation and return canonical errors.
- [ ] **SK-29-010** Facade builders preserve explicit zero/false/empty values and encode intent without duplicating IR defaults, normalization, schema validation, or unsupported-capability decisions.
- [ ] **SK-29-011** Search, YQL, typed-query, grouping, ranking, update, result convenience, and projection helpers preserve their canonical source data and reject unsupported sugar rather than silently approximating it.
- [ ] **SK-29-012** Ruby integer, symbol/string-key, encoding, truthiness, and mutable-object behavior cannot alter portable scalar types or request meaning.

## 29.3 Lifecycle, managed-local, and package bootstrap

- [ ] **SK-29-013** Create/open/delete lifecycle calls use canonical lifecycle variants and preserve structured AlreadyExists/NotFound/Conflict errors; handle close remains distinct from index deletion.
- [ ] **SK-29-014** Managed-local root ownership markers are created atomically, validated by exact type/version, and cannot be replaced or followed through symlinks.
- [ ] **SK-29-015** Legacy migration rejects ambiguous source/destination states and unsafe symlink/non-directory paths without following them.
- [ ] **SK-29-016** Equivalent concurrent migrations and identity-marker publication converge safely; unrelated or conflicting roots are never adopted or overwritten.
- [ ] **SK-29-017** Daemon/socket ownership prevents one Ruby handle from unlinking, terminating, or reusing another handle's resources; startup failure and abnormal exit clean owned state.
- [ ] **SK-29-018** Legacy single-index selectors are scrubbed when managed-local starts an explicitly multi-index daemon, without changing caller-global environment state unexpectedly.
- [ ] **SK-29-019** A clean gem build/install/autoload uses only packaged files and declared dependencies; package E2E does not rely on repository-only paths.

## 29.4 Evidence and real-engine path

- [ ] **SK-29-020** Every Ruby capability/runtime claim in `reference/client-contracts/catalog.json`, `docs/client-capabilities.json`, and the global capability ledger maps to executable conformance, ergonomic, or E2E evidence at the claimed layer.
- [ ] **SK-29-021** Semantic parity scenarios execute through Ruby's actual request/result conversion paths; parity is not inferred from source similarity.
- [ ] **SK-29-022** Registered Ruby HTTP and managed-local E2E commands reach the real Vespa-backed daemon through their production client paths and cover the advertised runtime behavior.
- [ ] **SK-29-023** Malformed/hostile BFBS-printed responses, transport failures, startup failures, and lifecycle races have negative-path or hardening evidence.
- [ ] **SK-29-024** Clean-package, unit, BFBS bridge, contract, parity, HTTP E2E, managed-local E2E, and required lifecycle-hardening commands are recorded with revision, result, and verifier non-goals.
