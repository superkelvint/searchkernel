# Search Kernel Developer Documentation — Unified Diátaxis and Generated Examples Plan

**Status:** Proposed implementation plan  
**Scope:** Developer-facing documentation for Search Kernel clients and the shared product API  
**Primary objective:** Make Search Kernel unusually easy and pleasant to learn and use while minimizing hand-maintained documentation drift

---

## 1. Product goal

Search Kernel should have **one documentation product**, not separate Python, TypeScript, PHP, Rust, Go, or Java documentation universes.

A developer learns Search Kernel once. The documentation explains one shared product model and one shared set of concepts. Wherever code is shown, the developer chooses the host language they are using.

The documentation experience should optimize for:

```text
find the right task quickly
        ↓
see the shortest correct explanation
        ↓
copy code that actually works
        ↓
understand the result
        ↓
discover the next useful capability naturally
```

Developer ergonomics and happiness are explicit product requirements.

The documentation must feel:

- fast to navigate;
- obvious to scan;
- consistent across languages;
- copy/pasteable;
- honest about capability availability;
- free of stale snippets;
- progressively disclosed;
- useful to both first-time and expert Search Kernel users.

The documentation is not merely prose describing the implementation. It is a verified product surface.

---

## 2. Core principles

### 2.1 One product, one documentation hierarchy

Do not create parallel documentation trees such as:

```text
/docs/python/filtering
/docs/typescript/filtering
/docs/php/filtering
```

Prefer:

```text
/docs/guides/filtering
```

with shared prose and language-selectable executable examples.

Language differences are presentation differences unless the capability itself is genuinely language-specific.

### 2.2 Diátaxis drives the information architecture

Use the Diátaxis modes as the design model:

```text
learning-oriented      tutorials
task-oriented          how-to guides
understanding-oriented explanation
information-oriented   reference
```

The public labels should remain developer-friendly rather than forcing users to learn documentation theory.

Recommended top-level navigation:

```text
Get started
Guides
Concepts
Reference
```

These correspond approximately to tutorials, how-to guides, explanation, and reference.

Do not mix the modes unnecessarily.

A guide that answers "How do I filter results?" should stay focused on doing the task and link to an explanation page for deeper semantics.

A concept page should explain why something works without pretending to be a step-by-step tutorial.

Reference should be terse, complete, and mechanically trustworthy.

### 2.3 Code examples are executable assets

Markdown prose must not be the primary owner of client code snippets.

Documented examples should come from source files that can be compiled, type-checked, or executed.

The default rule is:

```text
example source
    ↓
verification
    ↓
snippet extraction
    ↓
documentation rendering
```

not:

```text
Markdown code block
    ↓
hope it still works
```

### 2.4 Do not generate idiomatic example code from FlatBuffers

Search Kernel should not build a custom cross-language source-code generator merely to create documentation examples.

Each first-party SDK owns its idiomatic facade syntax. Documentation should display real idiomatic client code written in that language and verified by that language's normal tooling.

The generator renders and assembles documentation; it does not invent client ergonomics.

### 2.5 Generated protocol APIs are not the teaching surface

Normal documentation should teach the ergonomic Search Kernel facade:

```text
Index
SearchBuilder
PatchBuilder
UpdateBuilder
q
Group
agg
Hit
SearchResult
SearchKernelError
...
```

Generated FlatBuffers model types remain an advanced low-level escape hatch.

The documentation should mirror the product's intended progressive disclosure:

```text
simple search
    ↓
common search controls
    ↓
typed queries / grouping / ranking
    ↓
expert/raw escape hatches
```

---

## 3. Documentation information architecture

### 3.1 Get started

Keep the tutorial set deliberately small and high quality.

Initial tutorials:

```text
Build your first searchable index
Build product search
Add semantic/vector retrieval
```

A tutorial should be linear, opinionated, and designed to produce a successful outcome quickly.

The first tutorial should cover only enough to establish a useful mental model:

```text
install/connect
create or open an index
index a small believable dataset
search
filter
inspect results
```

Avoid introducing internal architecture, FlatBuffers, IR, or Vespa implementation details before the developer has successfully used the product.

### 3.2 Guides

How-to guides should be the largest part of the documentation.

Examples:

```text
Index documents
Bulk index documents
Get a document
Patch a document
Use conditional updates

Search text
Search selected fields
Filter search results
Sort results
Paginate
Select result fields
Highlight matches

Build facets
Group and aggregate
Use grouping continuations

Run phrase/proximity queries
Run fuzzy queries
Run nearest-neighbor search
Pass query inputs
Select ranking
Read match/summary features

Inspect a request before execution
Handle Search Kernel errors
Use YQL when the facade is not enough

Run searchkerneld locally
Connect to an existing daemon
Deploy the client in supported environments
```

Guides should prioritize the common path and omit theory that is not required to complete the task.

### 3.3 Concepts

Concept pages explain Search Kernel's model and the reasoning developers need to make good choices.

Initial concepts:

```text
How Search Kernel works
Indexes and documents
Full-text search vs structured filtering
search() vs query() vs yql()
Queries and ranking
Facets are grouping
Result trees
Persistence and reopen
Portable semantics across languages
Why non-Rust clients use a daemon
Generated low-level API vs ergonomic facade
```

Architecture details belong here when they improve the developer's mental model.

### 3.4 Reference

Reference should be generated as far as practical from public client code, canonical capability metadata, and stable product contracts.

Reference areas:

```text
Index
SearchBuilder
PatchBuilder
UpdateBuilder
AnalyzeBuilder
Query helpers
Grouping helpers
Aggregation helpers
Hit / SearchResult / ResultGroup
Errors
Configuration
Value types
Transport/configuration knobs
Low-level generated API
```

Reference pages should provide, where applicable:

- language-specific signatures;
- parameters;
- return values;
- error behavior;
- capability availability;
- short descriptions;
- links to executable examples;
- links upward to guides and concepts.

---

## 4. Global language selection

Language is a **global documentation preference**, not a separate documentation hierarchy.

The documentation UI should expose a persistent selector such as:

```text
Language: Python ▾

Python
TypeScript
PHP
Rust
Go
Java
...
```

### 4.1 Required behavior

When a developer selects TypeScript on one page:

1. all language-selectable code examples on that page switch to TypeScript;
2. navigation to another page keeps TypeScript selected;
3. reference signatures switch to TypeScript where applicable;
4. language-specific callouts switch accordingly;
5. the preference persists locally across visits.

The initial language may be inferred from an explicit language/package entry link, but the user must remain in control.

### 4.2 Local comparison is optional

Individual example blocks may offer a local language switcher for comparison, but the normal interaction is the single global preference.

Changing one local block for comparison must not unexpectedly rewrite the user's global preference unless the UI makes that action explicit.

### 4.3 Missing language support

Never silently fall back to another language.

If the selected language does not yet expose the ergonomic capability, render an honest state such as:

```text
Python ergonomic support for this operation is not available yet.

[Show low-level API] [Choose another language]
```

Where the capability is available through the generated low-level model, the page may offer that advanced escape hatch.

Availability must come from canonical capability evidence, not manually maintained page prose.

---

## 5. Semantic example IDs

Every reusable documentation example has a language-independent semantic ID.

Examples:

```text
quickstart.basic
documents.upsert
documents.patch
search.basic
search.filter.parameterized
search.sort.basic
search.pagination.page
search.fields.projection
query.phrase
query.nearest-neighbor
grouping.facet.basic
grouping.aggregate.avg
ranking.query-input
results.highlighting
errors.unknown-field
```

The ID means "the developer intent illustrated by this example", not a filename.

Each SDK may provide an implementation of the same example ID.

Conceptually:

```text
search.filter.parameterized

    python       implemented
    typescript   implemented
    php          implemented
    rust         implemented
    go           unavailable
```

This creates one conceptual example with multiple verified idiomatic presentations.

---

## 6. Example source layout

A likely source layout is:

```text
clients/python/examples/docs/
    quickstart.py
    documents.py
    filtering.py
    sorting.py
    grouping.py

clients/typescript/examples/docs/
    quickstart.ts
    documents.ts
    filtering.ts
    sorting.ts
    grouping.ts

clients/php/examples/docs/
    quickstart.php
    documents.php
    filtering.php

core/crates/searchkernel/examples/docs/
    quickstart.rs
    documents.rs
    filtering.rs
    ...
```

Exact paths may follow each language's conventions.

### 6.1 Snippet regions

A source file may expose extractable regions:

```python
# skdoc:begin search.filter.parameterized
results = (
    index.search("running shoes")
    .filter("price <= :max", {"max": 120})
    .limit(20)
    .execute()
)
# skdoc:end search.filter.parameterized
```

Equivalent syntax may be used in languages where comments differ.

The extracted region is what appears in the documentation, while the surrounding file may contain setup, fixtures, assertions, or cleanup required to make the example executable.

### 6.2 Complete runnable examples

Important examples should also be available as complete source files so a developer can run them without reconstructing omitted setup.

The rendered page may show only the interesting region and offer a "view complete example" action.

### 6.3 Believable data

Use realistic example domains such as products, articles, CMS content, locations, or inventory.

Avoid meaningless examples built around `foo`, `bar`, and `field1` unless the example is specifically about syntax independent of domain meaning.

---

## 7. Example verification classes

Not every documentation example requires a live Vespa engine.

Examples should declare one of the supported verification classes.

### 7.1 Compile / static

Proves that documented public syntax exists.

Examples:

- Python import/compile/type checks where available;
- TypeScript type checking;
- PHP syntax/static checks;
- Rust compilation.

### 7.2 Shared semantic

Uses recording/scripted/conformance infrastructure where appropriate to prove the facade constructs or interprets the expected canonical meaning without requiring real search execution.

### 7.3 Real-engine E2E

Runs through the supported product path and real Vespa backend.

Use E2E coverage for representative behavior where execution is important, including at least:

```text
quickstart
CRUD
basic search
filtering
sorting/pagination
projection/results
grouping/facets
highlighting
vector search where supported
ranking/query inputs where supported
```

Do not make every documentation build require every heavyweight native scenario.

---

## 8. Capability evidence integration

Search Kernel already has a canonical client capability evidence system built from the capability catalog and executable SKC annotations.

Documentation generation should extend that system rather than invent another support matrix.

The intended flow is:

```text
executable example/test
        │
        ├── capability evidence
        │
        └── documentation example
                │
                ▼
       docs rendering + support state
```

A documentation example can therefore simultaneously prove:

1. the public API exists;
2. the example compiles;
3. the example executes where required;
4. a client has ergonomic coverage;
5. the capability has a canonical developer-facing example;
6. the documentation is not advertising unsupported behavior.

The generator should consume the existing capability authorities, including:

```text
reference/client-contracts/catalog.json
docs/client-capabilities.json
SKC evidence annotations
```

Generated support documentation must not maintain an independent list.

---

## 9. Documentation content model

A small machine-readable documentation catalog may define page organization without duplicating API semantics.

Illustrative shape:

```yaml
sections:
  - id: search
    title: Search

  - id: filtering
    title: Filtering

pages:
  - id: guides.filtering
    mode: guide
    title: Filter search results
    capabilities:
      - filter.textual
    examples:
      - search.filter.basic
      - search.filter.parameterized
```

The catalog owns navigation and teaching structure only.

It must not define:

- Search Kernel defaults;
- validation semantics;
- field capability rules;
- query meaning;
- ranking meaning;
- result meaning;
- transport behavior.

Those remain in canonical product contracts and implementation.

---

## 10. API reference generation

API reference should be derived from each language's real public surface.

Potential inputs:

```text
Python       public symbols, signatures, type hints, docstrings
TypeScript   exported declarations and TSDoc
PHP          public classes/methods and PHPDoc
Rust         rustdoc/public API metadata
future SDKs  equivalent language-native metadata
```

Normalize this into a documentation intermediate representation containing concepts such as:

```text
symbol
language
kind
signature
summary
parameters
return type
errors
source location
related examples
capability IDs
```

The normal documentation generator then renders one conceptual reference page with the selected language's signature and details.

Do not use this metadata to redefine Search Kernel semantics.

---

## 11. Developer-experience requirements

These are product requirements, not optional polish.

### 11.1 Copy/pasteability

Important code should work when copied.

If setup is intentionally omitted, make that obvious and provide the complete runnable source.

### 11.2 Show outcomes

Examples should show useful representative output where seeing the output materially improves understanding.

Do not teach request construction while leaving the result model mysterious.

### 11.3 Happy path first

Lead with the most common successful form.

Put edge cases, raw APIs, and transport internals later or behind links.

### 11.4 Progressive disclosure

Do not make advanced Vespa capability harder to reach, but do not force advanced concepts into beginner paths.

### 11.5 Strong information scent

A page should make its purpose obvious immediately.

Prefer:

```text
Guides / Filtering

Filter search results
Restrict which matching documents are eligible.
```

over deep internal taxonomies.

### 11.6 Searchable terminology and aliases

Documentation search should recognize common intent and ecosystem vocabulary.

Examples:

```text
where
filter
filters
query filter
price range
structured filter
facet
aggregation
group by
score
relevance
projection
select fields
```

These aliases should lead to the canonical Search Kernel page.

### 11.7 Errors are discoverable

Machine-readable Search Kernel error codes should be searchable and linkable.

Where practical, error reference pages should link to the guides that commonly resolve them.

### 11.8 Cross-link the Diátaxis modes

Reference pages should link upward to guides and concepts.

Guides should link downward to relevant reference entries.

Concept pages should link to concrete guides demonstrating the idea.

Avoid an isolated reference dictionary.

---

## 12. Language-specific content

Shared prose is the default.

Language-specific content should be used only where the host language genuinely changes developer behavior.

Examples include:

- JavaScript safe-integer and `bigint` behavior;
- explicit TypeScript float helpers where host numeric syntax is ambiguous;
- Python context/process lifecycle idioms;
- PHP packaging/runtime constraints;
- language-specific async behavior;
- language-native duration/path/container types.

Render these as conditional callouts under the selected language rather than cloning the whole page.

---

## 13. Package README policy

Client package READMEs should not become parallel documentation sites.

Keep them intentionally small:

```text
package identity
installation
one executable quickstart
runtime/package-specific notes
brief capability/support pointer
link to canonical documentation
```

Large conceptual guides, deployment essays, or general Search Kernel explanations should move into the unified docs unless they are truly package-specific.

README snippets should reuse the same verified example source where practical.

---

## 14. Documentation build and verification

Provide one developer command for the client documentation pipeline, conceptually:

```bash
./dev docs clients
```

and one verification entry point, conceptually:

```bash
./dev verify client-docs
```

Exact command names should align with the repository's current verification DAG conventions.

### 14.1 Required failures

The verifier should fail closed for at least:

- duplicate semantic example IDs;
- referenced example IDs that do not exist;
- malformed or overlapping extraction regions;
- an example failing its declared compile/static check;
- a shared-semantic example failing its conformance check;
- an E2E-tagged example failing its required real-engine path;
- a documentation page advertising ergonomic support absent from capability evidence;
- capability/example annotations referring to unknown capability IDs;
- stale generated documentation when checked-in generated artifacts are policy;
- public reference symbols mentioned by generated docs that no longer exist;
- ambiguous or conflicting language registrations;
- documentation silently substituting another language when the selected language is unsupported.

### 14.2 Documentation health report

Generate a machine-readable and optionally human-readable coverage report.

Example:

```text
filter.textual
    reference            yes
    guide                yes
    python example       E2E
    typescript example   E2E
    php example          compile
    stale snippets       0
```

This should answer "what developer-facing documentation is actually missing?" without requiring a manual audit.

Do not turn this into a vanity percentage. Missing critical paths should remain visible by capability and language.

---

## 15. Verification DAG ownership

The client documentation generation/verification system is an independently meaningful subsystem and must be represented in the authoritative Search Kernel verification DAG before it becomes a merge gate.

At minimum define:

- a stable verification task/profile identity for client docs;
- the paths that select it;
- dependency edges to the client capability/evidence generation it consumes;
- lightweight vs E2E verification classes;
- full-suite reachability;
- fail-closed behavior for unknown documentation example sources or generated artifacts where applicable.

Do not rely on generic Markdown linting or broad CI to infer these semantics.

---

## 16. Suggested repository shape

Illustrative end state:

```text
docs/
    content/
        get-started/
        guides/
        concepts/
        reference/
    client-docs/
        catalog.yaml
        aliases.yaml

clients/
    python/
        examples/docs/
    typescript/
        examples/docs/
    php/
        examples/docs/

core/crates/searchkernel/
    examples/docs/

reference/
    client-contracts/
        catalog.json

tools/
    client-docs/
        extract-examples
        extract-api
        render
        verify
```

The exact implementation language and static-site framework are not fixed by this plan.

Prefer boring, standard tooling where it can satisfy the requirements.

---

## 17. Implementation sequence

### D0 — Freeze documentation architecture

Define and review:

- top-level information architecture;
- page modes and navigation vocabulary;
- global language selection behavior;
- semantic example ID rules;
- example verification classes;
- integration with existing capability evidence;
- documentation DAG ownership.

No documentation renderer should become authoritative before these contracts are clear.

### D1 — Example registry and extraction

Implement:

- semantic example registration;
- language registration;
- source-region extraction;
- compile/static verification;
- capability links;
- missing-language behavior;
- duplicate/stale detection.

Use a small proof set first:

```text
quickstart
CRUD
basic search
filtering
sorting
pagination
projection
```

Port existing executable client examples rather than rewriting behavior from scratch.

### D2 — Unified documentation shell

Implement:

- one shared site/navigation hierarchy;
- global persistent language selector;
- language-aware code blocks;
- conditional language callouts;
- honest unavailable-language rendering;
- copy code action;
- complete-example links;
- documentation search aliases.

### D3 — Diátaxis content migration

Create/migrate the initial high-value content:

```text
Get started:
    first searchable index
    product search

Guides:
    CRUD
    search
    filters
    sort
    pagination
    fields/projection
    highlighting
    facets/grouping
    vector search

Concepts:
    text vs filter
    search vs query vs YQL
    portable semantics
    result tree
    daemon model
```

Prefer shared prose with language-specific examples and callouts.

### D4 — Generated API reference

Add language-native API extraction and render shared conceptual reference pages.

Start with the main facade:

```text
Index
SearchBuilder
Hit
SearchResult
SearchKernelError
```

then expand to query/grouping/update helpers.

### D5 — Capability-aware documentation health

Generate documentation coverage/evidence reports.

Require that first-class ergonomic capabilities eventually have:

- reference;
- at least one verified example;
- a guide or intentional "reference only" classification;
- accurate language availability.

Do not manufacture examples for capabilities that are merely represented but not ergonomic.

### D6 — README consolidation and old-doc cleanup

Shrink per-client READMEs to package-specific entry material.

Remove or redirect duplicated language-specific guides after the unified versions are verified.

Do not delete useful historical material until an equivalent canonical page exists.

---

## 18. Initial acceptance criteria

The first useful documentation-system milestone is complete when:

1. there is one shared documentation hierarchy for Python, TypeScript, PHP, and Rust examples;
2. selecting a language persists across pages;
3. at least quickstart, CRUD, search, filtering, sorting, pagination, and projection use semantic example IDs;
4. those examples are extracted from executable source rather than copied Markdown;
5. each example passes its declared verification class;
6. unsupported selected languages render an honest unavailable state rather than fallback code;
7. capability availability comes from the existing canonical client capability evidence;
8. at least one Get Started tutorial, several Guides, several Concepts, and the initial Reference pages follow the Diátaxis-inspired structure;
9. the client-docs verifier exists in the authoritative verification DAG;
10. generated/reference output is reproducible and drift is rejected;
11. package READMEs point to the unified documentation rather than becoming separate documentation trees.

---

## 19. Non-goals

This plan does not authorize:

- redesigning Search Kernel client semantics;
- generating client implementations from documentation metadata;
- creating a Search Kernel-specific replacement for standard language documentation tooling without need;
- introducing a second capability ledger;
- translating one language's facade source into other languages automatically;
- exposing generated FlatBuffers objects as the primary teaching API;
- duplicating the same conceptual prose for every SDK;
- claiming an unsupported ergonomic capability merely because the low-level FlatBuffers model can represent it;
- making documentation generation depend on rebuilding Vespa for ordinary prose/reference changes.

---

## 20. Long-term quality bar

The intended end state is:

```text
one Search Kernel documentation product
        +
Diátaxis-inspired information architecture
        +
one persistent language choice
        +
shared conceptual prose
        +
idiomatic executable examples per language
        +
generated API reference
        +
canonical capability evidence
        +
CI that rejects stale documentation
```

A developer should be able to learn a Search Kernel concept once, switch examples to their language, copy working code, and trust that the documentation reflects the actual supported client surface.

That developer experience is part of the Search Kernel product.
