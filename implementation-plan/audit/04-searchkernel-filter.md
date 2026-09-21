# 4. Filter syntax parser — `core/crates/searchkernel-filter`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + UNIT/CONF + NEG`. This section proves syntax only; schema-aware binding is proven in §5.

## 4.1 Lexer

- [ ] **SK-04-001** Keyword matching is case-insensitive while field/parameter names preserve required case sensitivity.
- [ ] **SK-04-002** String escaping (`''`) is correct and source spans remain byte-accurate.
- [ ] **SK-04-003** Numeric literals preserve source text until schema-aware binding.
- [ ] **SK-04-004** Negative, exponent, integer, and floating syntax is accepted/rejected exactly as specified.
- [ ] **SK-04-005** Malformed tokens fail with deterministic structured spans.
- [ ] **SK-04-006** Token-count and expression-byte limits are enforced before pathological allocation/recursion.

## 4.2 Parser

- [ ] **SK-04-007** Operator precedence is exactly `NOT > AND > OR`.
- [ ] **SK-04-008** Parentheses override precedence correctly.
- [ ] **SK-04-009** `IN` accepts either a parameter list binding or explicit values as specified.
- [ ] **SK-04-010** Dangling operators, empty predicates, malformed `IN`, and unclosed groups fail deterministically.
- [ ] **SK-04-011** Recursive descent is bounded by the configured nesting limit.
- [ ] **SK-04-012** AST node count is bounded.
- [ ] **SK-04-013** Parser remains schema-independent and does not perform semantic field/type checks.

## 4.3 AST/error/limits

- [ ] **SK-04-014** Every syntax node needed for binding retains precise source spans.
- [ ] **SK-04-015** Complexity-limit errors map to stable error categories.
- [ ] **SK-04-016** Limits cover expression bytes, tokens, nesting, AST nodes, IN values, and parameter references.
- [ ] **SK-04-017** Limit-edge tests cover exactly-at-limit and one-over-limit cases.

## 4.4 Golden verification

- [ ] **SK-04-018** `searchkernel-filter/tests/golden.rs` is treated as **syntax/parser** evidence only; it does not prove schema-aware binding semantics.
- [ ] **SK-04-019** Positive grammar corpus covers equality, ranges, boolean composition, nested NOT, precedence, IN, parameter references, and numeric forms.
- [ ] **SK-04-020** Negative grammar corpus covers malformed syntax and all parser complexity limits.
- [ ] **SK-04-021** Golden parser fixtures are immutable acceptance inputs during implementation review.
- [ ] **SK-04-022** Schema-aware binding/equivalence evidence is checked separately under `searchkernel-ir::filter_expression`; never report a single "parser/binder golden" gate unless such a combined gate actually exists.

---
