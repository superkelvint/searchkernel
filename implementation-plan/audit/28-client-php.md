# 28. PHP client — `clients/php`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)

**Minimum evidence to PASS this section:** `SRC + UNIT + GEN + PKG`; each advertised PHP capability requires the corresponding production HTTP/FlatBuffers-JSON `E2E` evidence.

**Shared closure gates:** apply `SK-20-001..SK-20-013` where applicable, `SK-21-013`, `SK-22-002`, `SK-22-004`, `SK-23-016`, `SK-23-019`, `SK-26-001`, `SK-26-004`, `SK-26-007`, `SK-27-001`, `SK-27-004`, `SK-27-006`, `SK-27-007`, and `SK-27-009`; this module supplies PHP-specific source and evidence mapping without duplicating those obligations.

## 28.1 Canonical representation and values

- [ ] **SK-28-001** PHP request/response objects represent canonical FlatBuffers JSON table bodies and union discriminators; no independent domain DTO or operation-specific HTTP route exists.
- [ ] **SK-28-002** `emit_bfbs_contract.php` and `consume_bfbs_response.php` exercise representative PHP-produced requests and PHP-consumed responses through the official BFBS parser/printer.
- [ ] **SK-28-003** Empty objects remain objects, empty lists remain lists, and absent, null, false, and zero values retain distinct schema meanings through PHP array/object conversions and JSON encoding.
- [ ] **SK-28-004** Signed/unsigned integer boundaries, bytes, nested values, tensors, and non-finite portable values preserve their canonical union types or fail explicitly when PHP cannot represent them safely.
- [ ] **SK-28-005** Response decoding follows the declared value/result union discriminator and rejects unknown, malformed, or duplicate map entries without lossy coercion.
- [ ] **SK-28-006** Protocol version, request/response operation unions, lifecycle variants, and structured portable errors remain synchronized with the canonical FlatBuffers schema.

## 28.2 Transport, semantics, and builders

- [ ] **SK-28-007** HTTP transport sends standard FlatBuffers JSON only to the generic execute endpoint and does not parse or resolve Search Kernel domain semantics.
- [ ] **SK-28-008** HTTP status, timeout, redirect, invalid JSON, non-object response, and oversized or malformed response behavior fail deterministically as transport/protocol errors.
- [ ] **SK-28-009** Target strings are represented without a PHP-owned copy of canonical target grammar; invalid domain values reach server/IR validation and return canonical errors.
- [ ] **SK-28-010** Builders preserve explicit zero/false/empty values and encode intent without duplicating IR defaults, normalization, schema validation, or unsupported-capability decisions.
- [ ] **SK-28-011** Search, YQL, typed-query, grouping, ranking, update, and projection helpers encode only the variants they advertise; unsupported sugar is rejected clearly rather than silently approximated.
- [ ] **SK-28-012** PHP integer-width behavior, numeric-string handling, JSON key coercion, list/object ambiguity, and nullable fields are covered at protocol and facade boundaries.

## 28.3 Lifecycle and package bootstrap

- [ ] **SK-28-013** Create/open/delete lifecycle calls use canonical lifecycle request/response variants and preserve structured AlreadyExists/NotFound/Conflict errors.
- [ ] **SK-28-014** Closing a PHP client handle is local and does not delete, unload, or otherwise mutate shared daemon index state.
- [ ] **SK-28-015** PHP's lack of a managed-local runtime is explicit in the client contract/capability registry; package or example code does not imply daemon-spawn ownership.
- [ ] **SK-28-016** A clean Composer install/autoload uses only declared PHP/runtime requirements and shipped source, without repository-relative fallbacks or generated residue.
- [ ] **SK-28-017** The package consumer test validates the artifact users install, including public exports and protocol/facade usability.

## 28.4 Evidence and real-engine path

- [ ] **SK-28-018** Every PHP capability/runtime claim in `reference/client-contracts/catalog.json`, `docs/client-capabilities.json`, and the global capability ledger maps to executable conformance, ergonomic, or E2E evidence at the claimed layer.
- [ ] **SK-28-019** Semantic parity scenarios run through the PHP request builder and the shared canonical comparison harness; parity is not inferred from source similarity.
- [ ] **SK-28-020** The registered PHP HTTP E2E command reaches the real Vespa-backed daemon through the production client path and covers the advertised public operations/capabilities.
- [ ] **SK-28-021** PHP result decoding and errors are exercised with malformed/hostile BFBS-printed responses as well as successful real-engine responses.
- [ ] **SK-28-022** PHP clean-package, unit, BFBS bridge, contract, parity, and real-engine E2E commands are named in the audit record with revision, result, and verifier non-goals.
