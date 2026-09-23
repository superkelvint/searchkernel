# Search Kernel Developer Documentation — Unified Diátaxis and Generated Examples Plan

**Status:** Proposed implementation plan  
**Scope:** Developer-facing documentation for Search Kernel clients and the shared product API  
**Primary objective:** Make Search Kernel unusually easy and pleasant to learn and use while minimizing hand-maintained documentation drift

**Documentation framework:** [`docs/documentation-framework/`](../docs/documentation-framework/README.md) defines the authoritative site-level audiences, information architecture, content ownership, and governance. This plan implements the client-facing documentation system within that framework and must not create an alternative hierarchy.

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

## 17. Dogfooding documentation search with Search Kernel

Search Kernel documentation search should itself be a first-class Search Kernel application.

The purpose is not merely branding. The documentation search workload should continuously exercise real product surfaces and reveal developer-experience, relevance, deployment, client, persistence, and result-fidelity problems.

The core rule is:

> **Documentation search uses the normal Search Kernel product path. Do not create a docs-specific embedded search server or a second bespoke daemon.**

### 17.1 Hosting model

The documentation site is expected to be static and may be hosted on GitHub Pages.

GitHub Pages cannot host `searchkerneld`, so production search has two separately deployed pieces:

```text
GitHub Pages
docs.searchkernel.dev
        │
        │ browser request
        ▼
public Search Kernel serving edge
        │
        ▼
normal searchkerneld
        │
        ▼
verified docs index
```

The preferred long-term architecture is for the browser to use the normal supported browser/TypeScript client against a **generic restricted/read-only `searchkerneld` serving mode**.

Conceptually:

```text
GitHub Pages
    ↓
Search Kernel browser/TypeScript client
    ↓
HTTPS
    ↓
searchkerneld
    restricted/read-only serving policy
    ↓
searchkernel-docs index
```

This must be a general Search Kernel deployment capability, not a documentation-specific mode.

A serving policy may constrain, for example:

```text
allowed operations:
    search
    optionally get/analyze if later justified

allowed indexes:
    searchkernel-docs

disallowed:
    create
    schema mutation
    insert/upsert/patch/delete
    arbitrary administrative operations
```

The exact policy syntax is not fixed by this plan.

If Search Kernel cannot yet safely expose a public read-only index, that is a legitimate product/deployment ergonomics gap discovered by dogfooding. Prefer closing that general gap rather than bypassing it with a private docs-only server.

### 17.2 Temporary edge-proxy fallback

Until restricted/read-only public serving is available directly in `searchkerneld`, a stateless edge proxy may sit between GitHub Pages and a privately reachable `searchkerneld`.

Conceptually:

```text
GitHub Pages
    ↓
stateless edge proxy
    ↓
normal searchkerneld
    ↓
docs index
```

The proxy may own only edge/security policy such as:

- allowed origin/CORS;
- maximum query length;
- maximum result count;
- rate limiting;
- request timeout;
- fixed target index;
- allowed operation set.

The proxy must not own:

- indexing;
- persistence;
- ranking;
- filter semantics;
- query parsing semantics;
- result semantics;
- a second Search Kernel runtime;
- a docs-specific search implementation.

Its purpose is temporary containment of the public surface, not a second search service.

### 17.3 Search corpus contract

The docs build should emit a deterministic Search Kernel ingestion corpus as an ordinary generated artifact, for example:

```text
docs source
    ↓
docs generator
    ├── static site
    ├── API reference
    └── docs-search.jsonl
```

Search Kernel indexes the documentation model. It should not scrape rendered HTML or understand Markdown/MDX itself.

Index **semantic sections**, not arbitrary token chunks and not entire pages only.

A search document may represent:

- a meaningful H2/H3 section;
- an API reference symbol;
- an error code/reference entry;
- another independently navigable semantic unit.

Illustrative fields:

```text
id
url
page_id
title
heading
breadcrumbs
kind
diataxis_mode
body
aliases
capabilities
symbols
languages
version
code
```

Stable URLs and semantic IDs are required so a search result always lands at a meaningful location.

### 17.4 Lexical-first relevance

Start with excellent lexical developer search before adding semantic/vector retrieval.

Developer queries often contain high-value exact tokens such as:

```text
SearchBuilder
filter()
MissingPolicy
nearestNeighbor
SK_INVALID_FIELD
bigint
grouping
BM25
YQL
```

The initial relevance model should strongly prefer exact symbols, titles, headings, aliases, and then body/code content.

Hybrid/vector retrieval may be added later only after the lexical baseline is strong and the relevance suite demonstrates measurable benefit.

Do not make embeddings foundational merely because Search Kernel supports vectors.

### 17.5 Search Kernel features to dogfood

Use normal Search Kernel capabilities where they materially improve docs search:

- BM25/text matching for lexical retrieval;
- field-aware ranking for symbols, titles, headings, aliases, body, and code;
- highlighting for snippets;
- filters for documentation version, language-specific material, and content kind;
- grouping where useful to collapse multiple matching sections from the same page;
- facets where they improve exploration;
- stored fields/result projection for efficient result rendering;
- persistence/reopen for immutable deployed indexes;
- vector/hybrid retrieval later, if relevance evidence justifies it.

Do not use a feature merely to claim dogfood coverage.

### 17.6 Language preference and search

The global documentation language preference should influence search without hiding shared content.

For a TypeScript user, for example:

```text
shared prose              searchable normally
TypeScript-specific docs  preferred where relevant
Python/PHP-only details   deprioritized unless query intent requires them
```

A generic query such as `filter results` should still find the shared guide.

A language-specific query such as `bigint` should strongly surface the relevant TypeScript material.

### 17.7 Immutable documentation index deployments

Treat each documentation search index as an immutable deployment artifact.

Conceptually:

```text
build docs
    ↓
generate docs-search corpus
    ↓
create fresh Search Kernel index
    ↓
bulk ingest
    ↓
close
    ↓
reopen
    ↓
run relevance/acceptance suite
    ↓
publish verified index artifact
    ↓
deploy/promote
```

Production docs search should not depend on incremental content mutation during ordinary operation.

Each deployed index should carry a traceability manifest including at least:

```text
docs commit SHA
Search Kernel version/commit
schema version
corpus hash
document count
docs version
```

This makes deployment skew and relevance regressions diagnosable.

### 17.8 Relevance regression suite

Create a curated corpus of real developer-intent queries and expected destinations.

Examples:

```text
"filter by price"
    → filtering guide

"SearchBuilder filter"
    → SearchBuilder.filter reference

"typescript bigint"
    → TypeScript numeric-value documentation

"unknown field"
    → relevant error/reference page

"facet"
    → facets/grouping guide

"group by category"
    → grouping guide

"nearest neighbor"
    → vector-search guide

"update only one field"
    → patch-document guide

"yql"
    → YQL escape-hatch documentation
```

For strong navigational queries, the expected result should normally be rank 1.

For broader informational queries, define bounded top-N expectations.

Aggregate metrics such as MRR or NDCG may supplement these checks, but they must not replace understandable per-query acceptance expectations.

This suite should make relevance regressions visible whenever Search Kernel matching, ranking, analysis, grouping, highlighting, or result behavior changes.

### 17.9 Multi-client dogfooding

The production browser path should use the normal browser/TypeScript client where supported.

The same verified documentation index should also serve as a realistic client-conformance workload in CI/nightly verification:

```text
docs index
    ├── Rust smoke/relevance queries
    ├── TypeScript smoke/relevance queries
    ├── Python smoke queries
    └── PHP smoke queries
```

Equivalent portable requests should yield equivalent result identities and portable semantics where the clients support the same capability.

This complements small conformance fixtures with a meaningful real-world corpus.

### 17.10 Graceful degradation

Static documentation must not depend on the search service being healthy.

```text
page rendering/navigation   independent
documentation URLs          independent
API reference               independent
search                      enhanced service
```

A search outage may remove Cmd-K/search temporarily, but it must never make the documentation itself unavailable.

### 17.11 Dogfooding feedback is product evidence

When building or operating the docs search application exposes friction in:

- public read-only serving;
- browser transport;
- client ergonomics;
- index lifecycle;
- snapshot deployment;
- highlighting;
- grouping;
- ranking;
- result projection;
- errors;
- operational packaging;

treat that friction as evidence about the general Search Kernel product.

Do not paper over general product gaps with docs-specific semantic code.

### 17.12 Search dogfood rollout

#### DS0 — Corpus contract

Define semantic search-document units, stable IDs, metadata, aliases, language/version fields, and deterministic corpus generation.

#### DS1 — Lexical relevance

Implement and tune lexical search across title, heading, symbols, aliases, body, and code. Add highlighting and the initial relevance regression corpus.

#### DS2 — Production serving path

Serve the verified docs index through normal `searchkerneld`.

Prefer a generic restricted/read-only public serving mode. Until it exists, use only a stateless policy proxy in front of normal `searchkerneld`.

#### DS3 — Immutable index release pipeline

Build, close, reopen, verify, manifest, and promote immutable documentation index artifacts alongside the static GitHub Pages documentation release.

#### DS4 — Richer product features

Add grouping, facets, phrase/proximity/fuzzy behavior, and other Search Kernel features only where they improve actual documentation-search UX.

#### DS5 — Hybrid retrieval

Evaluate vector/hybrid retrieval against the existing lexical relevance suite. Adopt it only when it produces demonstrated relevance gains without regressing exact developer queries.

#### DS6 — Multi-client conformance workload

Use the same real docs index in Rust, TypeScript, Python, PHP, and future client verification as capabilities become executable/ergonomic.

---

## 18. Implementation sequence

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

## 19. Initial acceptance criteria

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
11. package READMEs point to the unified documentation rather than becoming separate documentation trees;
12. the docs build emits a deterministic semantic search corpus;
13. documentation search uses the normal Search Kernel/searchkerneld product path rather than a bespoke docs search daemon;
14. public search is restricted to an explicitly allowed read-only operation/index surface, either directly in searchkerneld or temporarily through a stateless policy proxy;
15. the deployed docs index is built, closed, reopened, and verified before promotion;
16. a curated relevance regression suite protects real developer-intent queries;
17. static documentation remains usable when the search service is unavailable.

---

## 20. Non-goals

This plan does not authorize:

- redesigning Search Kernel client semantics;
- generating client implementations from documentation metadata;
- creating a Search Kernel-specific replacement for standard language documentation tooling without need;
- introducing a second capability ledger;
- translating one language's facade source into other languages automatically;
- exposing generated FlatBuffers objects as the primary teaching API;
- duplicating the same conceptual prose for every SDK;
- claiming an unsupported ergonomic capability merely because the low-level FlatBuffers model can represent it;
- making documentation generation depend on rebuilding Vespa for ordinary prose/reference changes;
- creating a docs-specific embedded Search Kernel server or second bespoke search daemon;
- putting ranking, indexing, persistence, query semantics, or result semantics into a temporary edge proxy;
- exposing unrestricted public mutation/admin access merely to avoid designing a proper restricted/read-only serving surface.

---

## 21. Long-term quality bar

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
Search Kernel-powered documentation search
        +
normal searchkerneld deployment path
        +
relevance regression evidence
        +
CI that rejects stale documentation
```

A developer should be able to learn a Search Kernel concept once, switch examples to their language, copy working code, and trust that the documentation reflects the actual supported client surface.

That developer experience is part of the Search Kernel product.
