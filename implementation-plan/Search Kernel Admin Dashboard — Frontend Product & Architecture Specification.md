# Search Kernel Admin Dashboard

**Status:** Proposed frontend architecture  
**Primary UI reference:** Algolia Dashboard  
**Implementation stack:** React + TypeScript + Vite + Tailwind CSS + Tailwind Plus  
**Backend:** `searchkerneld` HTTP API  
**Product model:** Search Kernel concepts presented over the embedded Vespa engine

---

# 1. Purpose

The Search Kernel Admin Dashboard is the primary visual environment for inspecting, configuring, querying, debugging, and operating Search Kernel indexes.

It should feel immediately understandable to someone familiar with Algolia:

- compact;
- index-centric;
- search-oriented;
- highly interactive;
- developer-friendly;
- visually restrained;
- fast to navigate.

However, it must not inherit Algolia's underlying product model.

Search Kernel wraps Vespa's local C++ search engine and intentionally does not reproduce Vespa's distributed control plane, Java container architecture, cluster management, Config Server, distributed dispatch, or deployment model.

At the same time, Search Kernel exposes considerably richer search-engine concepts than a simple hosted-search configuration surface. The frontend must therefore have room for:

- document schemas;
- indexing behavior;
- attributes;
- document summaries;
- multiple ranking profiles;
- query inputs and ranking features;
- linguistic analysis;
- grouping/faceting;
- tensors and models where supported;
- engine diagnostics;
- configuration-impact planning.

The dashboard should make these capabilities understandable without forcing ordinary users to understand Vespa's deployment architecture.

---

# 2. Core product principle

The dashboard adopts:

> **Algolia for usability, Vespa for conceptual depth, Search Kernel for product vocabulary and semantics.**

The UI should not mirror Vespa.

It should also not flatten Search Kernel into Algolia's comparatively narrow model.

The frontend is a presentation and administration layer over the same Search Kernel APIs used by normal clients.

It must never become a second implementation of Search Kernel semantics.

The canonical application path remains:

```text
Dashboard
    ↓
generated TypeScript Search Kernel client
    ↓
HTTP/JSON transport
    ↓
searchkerneld
    ↓
generated JSON codec
    ↓
canonical FlatBuffers
    ↓
shared dispatcher
    ↓
searchkernel-core
    ↓
Search Kernel IR
    ↓
Vespa backend
```

This follows the existing C-series architecture, where HTTP/JSON is translated into canonical FlatBuffers before semantic processing.

No frontend code may independently reproduce:

- Search Kernel defaults;
- filter semantics;
- ranking semantics;
- schema validation;
- field capability validation;
- portable error definitions;
- request normalization.

---

# 3. Product object model

## 3.1 Global hierarchy

The user-visible hierarchy is deliberately simple:

```text
Search Kernel instance
    ├── Index
    ├── Index
    └── Index
```

A Search Kernel index is the principal unit of management.

It represents:

> One independently managed searchable document corpus, together with the schema, search configuration, ranking configuration, summaries, assets, and persistent engine state required to search it.

The dashboard does **not** expose artificial intermediate objects such as:

```text
Tenant
Application
Deployment
Zone
Cluster
Node
Content cluster
Container cluster
DocumentDB
```

Those concepts are either part of Vespa's distributed control plane or implementation details hidden beneath Search Kernel.

## 3.2 Future multiple-server support

The frontend architecture should not prevent a later connection model:

```text
Search Kernel connection
    └── indexes
```

But this should not be introduced as a user-facing hierarchy until there is a real requirement.

Do not invent an "organization," "project," or "application" abstraction simply to make the dashboard resemble a SaaS product.

---

# 4. Product scope

The dashboard has four major responsibilities.

## 4.1 Data administration

Users can:

- inspect documents;
- search documents;
- create/update/delete documents where supported;
- perform bulk operations where supported.

## 4.2 Search development

Users can:

- execute searches interactively;
- construct filters;
- configure ranking selection;
- choose projections/summaries;
- request facets/grouping;
- analyze text;
- inspect compiled queries;
- understand why results matched and ranked.

## 4.3 Configuration

Users can inspect and eventually modify:

- schema;
- fields;
- indexing behavior;
- rank profiles;
- summaries;
- search behavior;
- models/constants/assets.

## 4.4 Operations

Users can understand:

- engine/index health;
- document counts;
- storage use;
- memory use;
- search latency;
- ranking costs;
- summary/docsum latency;
- indexing activity;
- errors;
- engine/version information.

---

# 5. Explicit non-goals

The dashboard must not introduce UI concepts for functionality Search Kernel does not own.

Initially exclude:

- multi-node cluster topology;
- replication management;
- failover;
- partition assignment;
- Vespa application packages;
- Vespa container chains;
- Vespa query profiles as a product concept;
- Config Server;
- Slobrok;
- MessageBus;
- deployment zones;
- replicas;
- SaaS billing;
- organizations;
- API-key administration unless Search Kernel later defines authentication;
- analytics such as CTR/conversions unless Search Kernel explicitly gains an analytics event model.

Search Kernel is currently explicitly scoped as an embedded/local search engine rather than a distributed search platform.

---

# 6. Information architecture

The conceptual hierarchy is:

```text
SEARCH KERNEL
│
├── Overview
├── Indexes
│   │
│   └── <index>
│       │
│       ├── Overview
│       │
│       ├── Data
│       │   └── Documents
│       │
│       ├── Search
│       │   ├── Search Lab
│       │   ├── Analyze
│       │   └── Query Inspector
│       │
│       ├── Configuration
│       │   ├── Schema
│       │   │   ├── Fields
│       │   │   ├── Fieldsets
│       │   │   └── Indexing
│       │   │
│       │   ├── Ranking
│       │   ├── Summaries
│       │   ├── Search Behavior
│       │   └── Assets
│       │
│       └── Operations
│           ├── Health
│           ├── Query Performance
│           ├── Indexing & Storage
│           └── Diagnostics
│
├── Operations
└── Settings
```

This is the conceptual IA.

The entire hierarchy should **not** appear simultaneously in one giant sidebar.

---

# 7. Application shell

## 7.1 Global navigation

Persistent left sidebar:

```text
Search Kernel

Overview
Indexes

Operations
Settings

────────────────
v0.x.x
```

At the bottom:

- Search Kernel version;
- connection indicator;
- optionally Git commit/build metadata in a tooltip.

The global sidebar should remain narrow and stable.

Approximate width:

```text
240–260 px
```

## 7.2 Index workspace

After selecting an index:

```text
products                                      ● Ready
2.48m documents · 3.8 GB · updated 12 sec ago

Overview   Data   Search   Configuration   Operations
```

This secondary navigation belongs in the index workspace header.

The index name and state stay visible while moving between sections.

## 7.3 Section navigation

Sections with several functions receive a local secondary sidebar.

Example:

```text
CONFIGURATION

Schema
Ranking
Summaries
Search Behavior
Assets
```

Search:

```text
SEARCH

Search Lab
Analyze
Query Inspector
```

Operations:

```text
OPERATIONS

Health
Query Performance
Indexing & Storage
Diagnostics
```

This preserves conceptual depth without overwhelming primary navigation.

---

# 8. Route architecture

Use explicit routes rather than storing application navigation in global component state.

Recommended structure:

```text
/

/overview

/indexes
/indexes/new

/indexes/:indexId
/indexes/:indexId/overview

/indexes/:indexId/data
/indexes/:indexId/data/documents
/indexes/:indexId/data/documents/:documentId

/indexes/:indexId/search
/indexes/:indexId/search/lab
/indexes/:indexId/search/analyze
/indexes/:indexId/search/inspect

/indexes/:indexId/configuration
/indexes/:indexId/configuration/schema
/indexes/:indexId/configuration/schema/fields
/indexes/:indexId/configuration/schema/indexing
/indexes/:indexId/configuration/schema/fieldsets
/indexes/:indexId/configuration/ranking
/indexes/:indexId/configuration/ranking/:profile
/indexes/:indexId/configuration/summaries
/indexes/:indexId/configuration/summaries/:summary
/indexes/:indexId/configuration/search-behavior
/indexes/:indexId/configuration/assets

/indexes/:indexId/operations
/indexes/:indexId/operations/health
/indexes/:indexId/operations/query-performance
/indexes/:indexId/operations/storage
/indexes/:indexId/operations/diagnostics

/operations
/settings
/about
```

Canonical URLs should redirect to the default child view.

For example:

```text
/indexes/products/search
```

becomes:

```text
/indexes/products/search/lab
```

---

# 9. URL state

Important user state should survive reloads and browser navigation.

For Search Lab:

```text
?q=wireless+headphones
&ranking=product-search
&limit=20
```

For document browsing:

```text
?query=sony
&sort=price:asc
&cursor=...
```

Complex request state does not need to be encoded completely into the URL.

The URL should contain enough state to restore the principal view.

Large or complicated Search Lab configurations may additionally support a future saved-search/preset mechanism.

---

# 10. Global Overview

The global homepage answers:

1. Is Search Kernel healthy?
2. Which indexes exist?
3. Which index requires attention?

Layout:

```text
Search Kernel

┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐
│ 3 indexes  │ │ 2.6m docs  │ │ 4.4 GB     │ │ ● Healthy  │
└────────────┘ └────────────┘ └────────────┘ └────────────┘

Indexes

Name          Documents      Size       Status        Last activity
products      2,482,192      3.8 GB     ● Ready       12 sec ago
articles         81,402      440 MB     ● Ready       4 min ago
users            12,832       91 MB     ● Ready       11 min ago
```

Clicking an index navigates into its workspace.

The page may also contain a subdued engine-information section:

```text
Search Kernel          0.x.x
Portable API           0.x
Native ABI             0.x
Vespa revision         abc123...
Uptime                  4d 3h
```

Avoid dashboard filler such as fake activity streams.

---

# 11. Index Overview

Purpose:

> Give the user a useful status summary before they enter a specialist workspace.

Header statistics:

```text
Documents
2,482,192

Index size
3.8 GB

Fields
18

Search latency
3.7 ms p50
```

Recommended sections:

### Schema summary

Show representative fields and capabilities.

### Ranking

Show available ranking profiles:

```text
default
product-search
semantic
```

### Recent engine activity

Only show this if real events exist.

Examples:

- reopened;
- schema changed;
- indexing completed;
- failure recovered.

### Quick Search

A compact search field:

```text
[ Search products...                          ]
```

Submitting opens Search Lab.

---

# 12. Data → Documents

This is the principal corpus browser.

It should feel close to Algolia Browse.

## 12.1 Toolbar

```text
[ Search documents...                         ]

[ Filters ] [ Columns ] [ View ▾ ]       [ + Document ]
```

Potential view modes:

```text
Table
Cards
JSON
```

Table is the default.

## 12.2 Table

Example:

```text
_id       title                     brand      price      stock
sku-42    Wireless Headphones       Sony       $149       17
sku-43    Studio Headphones         Bose       $229       4
```

Support:

- configurable visible columns;
- horizontal scrolling;
- sortable columns where legal;
- selection;
- pagination/cursor controls;
- bulk action toolbar when rows are selected.

Do not pretend every field is sortable.

Field capabilities determine available operations.

## 12.3 Filtering

Structured filters should be generated from schema capabilities.

For example:

```text
brand       equals / not equals
price       = / < / <= / > / >= / between
in_stock    true / false
```

Do not offer unsupported operators and wait for the backend to reject them.

The backend remains authoritative, but schema capability information should drive the UI.

## 12.4 Document drawer

Selecting a document opens a right-side drawer.

```text
Document
sku-42

[ Fields ] [ JSON ]

title
Wireless Headphones

brand
Sony

price
149

...

────────────────
[ Edit ] [ Delete ]
```

Avoid leaving the corpus browser merely to inspect one record.

The document URL may still update so the state is deep-linkable.

## 12.5 Editing

Editing modes:

```text
Form
JSON
```

Form mode is generated from known scalar field types where practical.

JSON mode handles complex documents.

Mutations should not use optimistic UI for authoritative document state.

Flow:

```text
submit
 ↓
backend acknowledges
 ↓
invalidate relevant query cache
 ↓
refresh
```

## 12.6 Delete

Delete requires a confirmation dialog.

Single-document delete does not require typing the ID unless there is a good usability reason.

Index deletion does.

---

# 13. Dashboard display preferences

Document display configuration is **not index search semantics**.

Possible preferences:

```text
Primary label field     title
Secondary field         brand
Image field             image_url
Badge fields            category, in_stock
Default table columns   title, brand, price, stock
```

This data must not become part of `IndexDefinition`.

Initially store it in:

```text
localStorage
```

keyed by:

```text
server identity + index identity
```

A future small dashboard-metadata service may persist it across browsers.

Keep that system completely separate from the portable Search Kernel schema.

---

# 14. Search → Search Lab

Search Lab should be the centerpiece of the dashboard.

It is not simply a large search box.

It is an interactive visual debugger for the complete Search Kernel query pipeline.

Desktop layout:

```text
┌──────────────────────────────────────────────────────────────────────┐
│ [ wireless headphones                                    ] [Search] │
├───────────────────┬──────────────────────────┬───────────────────────┤
│ REQUEST           │ RESULTS                  │ INSPECTOR             │
│                   │                          │                       │
│ Search fields     │ 1. Product A      12.41  │ Request               │
│ Filters           │                          │ Resolved Request      │
│                   │ 2. Product B      11.88  │ Query                 │
│ Ranking           │                          │ Ranking               │
│                   │ 3. Product C       9.44  │ Features              │
│ Results           │                          │ Timings               │
│                   │                          │ Raw                   │
│ Facets            │                          │                       │
└───────────────────┴──────────────────────────┴───────────────────────┘
```

Recommended column proportions:

```text
Request        300–360 px
Results        flexible
Inspector      360–440 px
```

Both side panels should be independently collapsible.

---

# 15. Search Lab request builder

Organize the builder according to the actual search pipeline.

## Query

```text
Query text
Query expression
Structured filters
```

## Retrieval

```text
Search fields
Field weights
Vector retrieval when supported
```

## Ranking

```text
Rank profile
Query inputs
```

## Results

```text
Summary/projection
Explicit fields
Sort
Limit
Offset/cursor
```

## Aggregation

```text
Facets
Groups
Aggregations
```

## Debug

```text
Inspect query
Feature output
Trace/debug options
```

Sections should be collapsible.

Frequently used controls stay expanded.

Advanced controls start collapsed.

---

# 16. Search execution behavior

Search should support two modes.

### Explicit

Click:

```text
Search
```

or press:

```text
Ctrl/Cmd + Enter
```

### Live

Optional toggle:

```text
Live search
```

When enabled:

- debounce input;
- cancel in-flight requests using `AbortSignal`;
- ensure stale responses cannot replace newer results.

Do not default expensive vector/grouping/debug queries to fire on every keystroke unless performance makes this clearly safe.

---

# 17. Search result card

Each hit should expose:

```text
rank
score
primary document label
selected fields
badges
```

Example:

```text
1                                              12.418
Wireless Headphones
Sony · Electronics

$149        In stock

[ Why this result? ]
```

Clicking opens either:

- document drawer;
- result-inspection drawer.

The two functions should remain distinguishable.

---

# 18. "Why this result?"

This should become a major Search Kernel differentiator.

The frontend must not fabricate explanations.

Render backend-provided information such as:

```text
Matched fields
Analyzed terms
Score
Ranking profile
Ranking features
Sort values
Filters
```

Eventually:

```text
BM25 feature values
first-phase score
second-phase score
query features
attribute features
```

where available.

The UI should tolerate partial explanation support.

A capability may expose:

```text
result_explanation.basic
```

without:

```text
result_explanation.ranking_features
```

---

# 19. Search Lab Inspector

The inspector should reveal the Search Kernel pipeline.

Tabs:

```text
Request
Resolved
Analysis
Query
Ranking
Timings
Raw
```

## Request

What the caller expressed.

## Resolved

What Search Kernel resolved through `*_to_ir()`.

This is particularly important because the project's architecture deliberately distinguishes portable caller intent from explicit Search Kernel IR.

## Analysis

Linguistic output for relevant terms.

## Query

Compiled/structured query representation available from the backend.

## Ranking

Selected profile and feature information.

## Timings

Pipeline timing information where available.

## Raw

Canonical response/request material suitable for debugging.

---

# 20. Builder and Raw Request modes

Search Lab should expose:

```text
Builder | JSON
```

The JSON view represents the public Search Kernel HTTP/JSON request.

It must use the generated canonical JSON representation defined by the C-series contract rather than a dashboard-specific request syntax.

Provide:

```text
Format
Copy JSON
Copy curl
```

The eventual JSON editor should use:

```text
CodeMirror 6
```

rather than Monaco unless later requirements justify Monaco's significantly larger footprint.

The generated JSON Schema/OpenAPI-compatible metadata from C6 can eventually provide:

- autocomplete;
- validation;
- enum completion;
- inline documentation.

---

# 21. Analyze

Analyze belongs inside Search rather than being a primary index section.

Purpose:

> Show how user text becomes searchable/query terms.

Layout:

```text
Analyze text

Field / analyzer
[ title / standard-v1                        ▾ ]

Input
┌─────────────────────────────────────────────────────┐
│ Running with the foxes                              │
└─────────────────────────────────────────────────────┘

[ Analyze ]
```

Output:

```text
Position   Original     Normalized     Term
0          Running      running        run
1          with         with           with
2          the          the            the
3          foxes        foxes          fox
```

Only display transformations Search Kernel actually exposes.

Avoid frontend approximations.

M3 explicitly establishes document- and query-side linguistic compatibility against Vespa behavior.

---

# 22. Query Inspector

The Query Inspector is a dedicated deeper debugging surface.

It complements Search Lab rather than replacing it.

Potential sections:

```text
Portable request
Resolved Search Kernel IR
Analyzed terms
Compiled query representation
Ranking inputs
Ranking profile
Execution information
Response
```

Primary use cases:

```text
Why did this query resolve this way?
Why did this field participate?
Why was this analyzer selected?
Why didn't a document appear?
Why did ranking profile X run?
```

`InspectQueryRequest` is already part of the planned portable API surface.

This page should make that capability visually useful.

---

# 23. Configuration shell

Configuration pages share a local navigation:

```text
CONFIGURATION

Schema
Ranking
Summaries
Search Behavior
Assets
```

Each configuration page follows a common pattern:

```text
Title
description                                      [ Edit ]

status/impact information

main configuration content
```

Editing never modifies configuration silently.

Changes pass through validation and an impact phase.

---

# 24. Configuration change lifecycle

This should be a first-class dashboard concept.

Workflow:

```text
Current configuration
        ↓
Edit draft
        ↓
Validate
        ↓
Generate change plan
        ↓
Review impact
        ↓
Apply
        ↓
Progress/state
        ↓
Completed / failed
```

Example:

```text
Change plan

title.stemming
best → none

Required actions
────────────────────────────────

Configuration update       Required
Index rebuild              title
Document refeed            Not required
Engine reopen              Required
Expected data loss         None

[ Cancel ]                         [ Apply ]
```

The dashboard must never infer this impact itself.

The backend must return the plan.

This becomes especially important as schema evolution becomes richer.

---

# 25. Configuration dirty state

When editing:

```text
Saved configuration
Draft
```

must be distinct.

Leaving a route with unsaved configuration should produce a navigation warning.

A backend apply failure leaves the draft intact.

Do not automatically discard user work.

---

# 26. Configuration → Schema

Schema concerns how documents are represented for search.

Subnavigation:

```text
Fields
Fieldsets
Indexing
```

M6's low-level supported schema includes text/string/integer/floating-point/boolean and index/attribute/stored capabilities.

The frontend should be capable of growing beyond that initial subset without redesign.

---

# 27. Schema → Fields

Table:

```text
Field          Type       Index      Attribute      Summary      Analyzer
title          text       ✓          —              ✓            standard
brand          string     ✓          ✓              ✓            —
price          float      —          ✓              ✓            —
embedding      tensor     —          ✓              —            —
```

The exact terminology should follow Search Kernel's public schema vocabulary.

Click a field for details.

---

# 28. Field details

Right drawer or dedicated route:

```text
title

Type
Text

Search capabilities
Searchable          Yes
Attribute           No
Stored              Yes

Analysis
Analyzer            standard-v1
Stemming            best

Ranking
BM25                 Enabled
```

Where relevant, include an explanatory section:

```text
Used for

✓ full-text matching
✓ returned in summaries
✓ ranking

Not available for

— sorting
— grouping
```

This is important usability.

The dashboard should help users understand **what field configuration enables**, not merely display flags.

---

# 29. Schema field editor

Conceptual form:

```text
Name
[ title ]

Type
[ Text ▾ ]

Storage and indexing
☑ Searchable
☑ Stored
☐ Attribute

Analyzer
[ Standard v1 ▾ ]

Advanced
...
```

Illegal combinations should be disabled or explained where known.

Backend validation remains authoritative.

---

# 30. Fieldsets

Fieldsets become useful once exposed by Search Kernel.

Example:

```text
default-search

Fields
✓ title
✓ description
✓ brand
```

The UI should present fieldsets as reusable named sets, not require users to write Vespa schema syntax.

If Search Kernel never exposes fieldsets at the high-level product layer, hide this navigation entry.

---

# 31. Indexing configuration

This area describes document-to-index processing.

It may expose:

```text
field indexing behavior
linguistics
indexing pipeline
attribute behavior
```

Avoid exposing Vespa indexing-language syntax merely because the underlying engine uses it.

If Search Kernel eventually exposes an advanced raw representation, put that behind:

```text
Advanced / Engine representation
```

and keep it secondary.

---

# 32. Configuration → Ranking

Ranking is first-class.

Do not call this page simply "Relevance."

Primary object:

```text
Rank Profile
```

Layout:

```text
Ranking Profiles

● default
  product-search
  semantic
  recommendations

[ + Profile ]
```

Selecting a profile opens its configuration.

---

# 33. Ranking profile page

Recommended conceptual structure:

```text
product-search

General
Inputs
Retrieval / matching interaction
First phase
Second phase
Features
Models
Advanced
```

The exact available sections are capability-driven.

Example:

```text
Inputs

query(category_boost)
double

query(user_embedding)
tensor<float>(x[384])
```

Ranking execution remains Vespa's responsibility; Search Kernel exposes and translates supported capability rather than implementing its own ranking engine.

---

# 34. High-level versus low-level ranking

Maintain the roadmap's architectural distinction:

```text
Search Kernel ranking policy
        ↓
integration lowering
        ↓
Vespa ranking
```

Normal dashboard editing should primarily expose Search Kernel-supported ranking concepts.

Low-level Vespa ranking information may appear under:

```text
Engine details
```

or debugging/inspection.

Do not make ordinary dashboard configuration depend on manually writing arbitrary Vespa rank-expression syntax unless that explicitly becomes part of the Search Kernel product API.

---

# 35. Ranking comparison

A valuable future Search Lab feature:

```text
Compare ranking

Query:
wireless headphones

A: default
B: product-search
```

Results displayed side-by-side.

Potential comparison output:

```text
rank changes
score changes
documents introduced
documents removed
feature differences
```

This should not block the initial dashboard.

The architecture should merely leave room for it.

---

# 36. Configuration → Summaries

Document summaries deserve a dedicated section.

They control result projections and may have important execution/performance consequences.

List:

```text
Summary

default
search-result
compact
autocomplete
```

Selecting one:

```text
search-result

Output field      Source                 Strategy
title             title                  attribute
brand             brand                  attribute
price             price                  attribute
description       description            stored/dynamic
```

Where backend information permits, visually identify expensive access.

For example:

```text
Performance

3 fields served from attributes
1 field requires document-store access
```

Do not manufacture this determination in JavaScript if the backend cannot provide it reliably.

---

# 37. Search Lab summary selection

The Search Lab request builder should allow:

```text
Result summary
[ search-result ▾ ]
```

Selecting a summary should update the result rendering automatically.

This makes summary configuration testable immediately.

---

# 38. Configuration → Search Behavior

This section holds Search Kernel-owned higher-level behavior.

Potential children:

```text
Presets
Synonyms
Curations
Prefix behavior
Typo behavior
Defaults
```

Only render supported capabilities.

These correspond naturally to the higher-level search behavior planned in the integration roadmap.

Do not introduce Vespa Query Profiles as a product concept merely because Vespa has them.

---

# 39. Presets

A preset is a named Search Kernel search configuration.

Example:

```text
product-search

Ranking          product-search
Summary          search-result
Limit            20
Facets            brand, category
```

The exact preset semantics remain owned by Search Kernel core.

The frontend only edits the public representation.

---

# 40. Synonyms

When supported:

```text
Synonym sets

laptop
  notebook
  portable computer
```

Provide:

- create;
- edit;
- delete;
- search;
- import/export where justified.

Search Kernel defines the semantics.

The dashboard must not implement query rewriting independently.

---

# 41. Curations

Potential UI:

```text
Query
"wireless headphones"

Pinned
1  sku-42
2  sku-18

Hidden
sku-993
```

Again, this becomes UI over Search Kernel-owned behavior rather than frontend manipulation of result arrays.

---

# 42. Configuration → Assets

Initially hidden unless capabilities require it.

Potential resources:

```text
Constants
Models
```

Later possibly:

```text
ONNX models
tensor constants
other ranking assets
```

Show:

```text
name
type
size
used by
status
```

Asset lifecycle should be explicit.

Never make binary model uploads appear to succeed until backend persistence acknowledges them.

---

# 43. Index Operations

Operations is not generic "analytics."

It exposes observable state of the local search engine.

Subnavigation:

```text
Health
Query Performance
Indexing & Storage
Diagnostics
```

---

# 44. Operations → Health

Purpose:

> Is this index operational and is anything obviously wrong?

Top status:

```text
● Ready
```

Potential metrics:

```text
Documents
Searchable documents
Removed documents

Memory
Disk

Transaction log
Last successful write
Last successful search
```

Warnings appear as actionable notices rather than merely changing chart colors.

Example:

```text
Index memory increased 42% over the last hour.

[ View storage details ]
```

Only surface warnings backed by actual rules defined by the server/product.

---

# 45. Operations → Query Performance

Organize by engine/query pipeline rather than dumping metrics.

Conceptual visualization:

```text
Query
  ↓
Matching
  ↓
Ranking
  ↓
Reranking
  ↓
Summary
```

Metrics may include:

```text
request latency
matching latency
documents matched
documents ranked
documents reranked
reranking time
summary latency
```

Display:

```text
p50
p95
p99
```

where the metrics source supports those statistics.

Support filtering/grouping by:

```text
ranking profile
```

when available.

This will be substantially more actionable than one global "search latency" chart.

---

# 46. Operations → Indexing & Storage

Potential metrics:

```text
documents total
documents searchable
documents removed

disk usage
memory index size
attribute memory
document-store usage
transaction-log usage

writes/sec
updates/sec
deletes/sec
commit latency
```

Charts should have useful time windows:

```text
15m
1h
6h
24h
```

Longer retention depends on metrics persistence.

Do not imply historical metrics exist if the daemon only stores recent data.

---

# 47. Operations → Diagnostics

This is the deep technical view.

Potential information:

```text
Search Kernel version
portable API version
native ABI version
Vespa revision
platform
uptime

engine state
open directory/path
configuration generation/version
recent engine errors
```

Provide:

```text
Copy diagnostics
```

which produces a redacted troubleshooting bundle/string.

Never include document contents or secrets automatically.

---

# 48. Global Operations

Global Operations summarizes the daemon.

Potential sections:

```text
Process health
CPU
RSS
disk

Open indexes
request volume
error rate

versions
uptime
```

This page answers:

> Is the Search Kernel process healthy?

Index Operations answers:

> Is this particular index healthy?

---

# 49. Settings

Keep Settings deliberately small.

Initial candidates:

```text
Appearance
Dashboard preferences
Connection information
Diagnostics
```

Avoid turning Settings into a dumping ground for index configuration.

Index behavior belongs under its index.

---

# 50. Capability negotiation

Capability-driven rendering is a hard architectural requirement.

The UI must not infer support from release numbers.

Bad:

```typescript
if (version >= "0.9.3") {
  showRankingFeatures();
}
```

Good conceptually:

```typescript
if (capabilities.ranking.featureInspection) {
  showRankingFeatures();
}
```

The server should expose:

```text
server version
portable API version
supported operations
optional capabilities
```

Capabilities should be granular enough to express partial support.

Example conceptual hierarchy:

```text
schema.read
schema.write
schema.changePlan

documents.read
documents.write
documents.bulk

search.basic
search.sort
search.facets
search.grouping

analysis.text
query.inspect

ranking.profiles
ranking.features
ranking.models

summaries.read
summaries.write

metrics.query
metrics.storage
```

This is conceptual naming, not a frozen wire format.

The actual representation should belong to the Search Kernel API.

---

# 51. Navigation capability behavior

If a whole feature is unsupported:

```text
hide it
```

rather than littering navigation with:

```text
Coming soon
```

If a feature exists but is read-only:

```text
show it
disable mutation actions
```

and explain why.

Example:

```text
Schema editing is not available for this index.

This index was opened using a configuration that cannot be modified through the current backend.
```

---

# 52. Portable API versus admin API

Maintain two clear frontend backend domains.

## Portable Search Kernel API

Used for semantics that are part of the product:

```text
index lifecycle
schema
documents
search
filters
ranking
facets
summaries
analyze
inspect query
search behavior
```

These should go through the generated Search Kernel TypeScript client.

## Administrative/diagnostic API

Used for process-local concerns:

```text
uptime
RSS
disk usage
build metadata
native SDK information
recent process errors
metrics snapshots
```

These may use a small explicitly separate admin client.

Do not add `/dashboard/...` semantic endpoints merely because implementing them feels convenient.

---

# 53. Backend capabilities required by the dashboard

The dashboard eventually requires operations conceptually equivalent to:

```text
list indexes
describe index
create index
open index
close index
delete index

browse documents
get document
write document
delete document

search
analyze
inspect query

describe schema
validate schema
plan configuration change
apply configuration change

list ranking profiles
describe ranking profile

list summaries
describe summary

retrieve index metrics
retrieve process metrics
retrieve diagnostic metadata
```

Where these operations do not yet exist, add them to the appropriate Search Kernel API layer rather than inventing frontend workarounds.

---

# 54. Change planning backend contract

The frontend should eventually receive a structured result conceptually like:

```text
valid: true

actions:
  - kind: reconfigure
  - kind: reindex
    fields: [title]
  - kind: reopen

warnings:
  - ...

destructive: false
```

The dashboard then renders that information.

The server decides:

```text
what needs rebuilding
what requires reopening
what is destructive
what is unsupported
```

The frontend never deduces this from changed fields.

---

# 55. Frontend technology stack

Recommended:

```text
React
TypeScript
Vite

Tailwind CSS
Tailwind Plus

TanStack Router
TanStack Query

React Hook Form

CodeMirror 6

Heroicons

Vitest
React Testing Library
Playwright
```

Avoid:

```text
Next.js
Redux
large server-rendering framework
frontend-specific backend service
```

unless later requirements materially change.

The dashboard has no compelling SSR requirement.

---

# 56. Deployment architecture

Production:

```text
searchkerneld
    ├── /api/...
    └── dashboard static assets
```

Browser:

```text
http://localhost:xxxx/
```

Static assets:

```text
dashboard/dist/
├── index.html
└── assets/
```

This keeps the low-operations product model intact.

No separate Node process is required.

---

# 57. Development architecture

Development:

```text
Vite development server
    ↓
proxy /api
    ↓
searchkerneld
```

Example:

```text
localhost:5173
       ↓
/api/*
       ↓
localhost:8080
```

Production remains same-origin.

---

# 58. Frontend source structure

Recommended:

```text
dashboard/
├── package.json
├── vite.config.ts
├── tsconfig.json
├── src/
│   │
│   ├── app/
│   │   ├── router.tsx
│   │   ├── providers.tsx
│   │   ├── errors.tsx
│   │   └── shell/
│   │       ├── AppShell.tsx
│   │       ├── GlobalSidebar.tsx
│   │       ├── IndexHeader.tsx
│   │       ├── IndexNavigation.tsx
│   │       └── SectionNavigation.tsx
│   │
│   ├── routes/
│   │   ├── overview/
│   │   ├── indexes/
│   │   ├── index-overview/
│   │   ├── documents/
│   │   ├── search-lab/
│   │   ├── analyze/
│   │   ├── query-inspector/
│   │   ├── schema/
│   │   ├── ranking/
│   │   ├── summaries/
│   │   ├── search-behavior/
│   │   ├── assets/
│   │   ├── index-operations/
│   │   ├── global-operations/
│   │   └── settings/
│   │
│   ├── features/
│   │   ├── indexes/
│   │   ├── documents/
│   │   ├── search-lab/
│   │   ├── analysis/
│   │   ├── query-inspection/
│   │   ├── schema/
│   │   ├── ranking/
│   │   ├── summaries/
│   │   ├── change-plans/
│   │   └── diagnostics/
│   │
│   ├── components/
│   │   ├── ui/
│   │   ├── data/
│   │   ├── forms/
│   │   ├── layout/
│   │   └── feedback/
│   │
│   ├── api/
│   │   ├── searchkernel-client.ts
│   │   ├── admin-client.ts
│   │   ├── query-keys.ts
│   │   └── capabilities.ts
│   │
│   └── lib/
│       ├── formatting.ts
│       ├── urls.ts
│       ├── storage.ts
│       └── errors.ts
│
└── tests/
    ├── unit/
    └── e2e/
```

Organize primarily by product feature.

Avoid abstract structures such as:

```text
atoms
molecules
organisms
```

---

# 59. Tailwind Plus integration

Tailwind Plus should be treated as a component source and visual foundation, not pasted independently into every page.

Normalize selected Tailwind Plus patterns into internal components:

```text
Button
IconButton
Input
Textarea
Select
Combobox
Checkbox
RadioGroup
Switch

Tabs
Badge
Tooltip
Dropdown

Table
DataTable

Modal
Drawer
AlertDialog

Alert
EmptyState
ErrorPanel

PageHeader
SectionHeader
DescriptionList

CommandPalette
```

Page-specific components use these primitives.

Do not create an unnecessary independent design system.

---

# 60. Visual language

Target:

> Algolia-like engineering dashboard rather than generic modern SaaS.

Characteristics:

```text
light neutral background
white primary work surfaces
thin borders
minimal shadow
compact controls
restrained indigo/violet accent
high information density
strong table UX
```

Avoid:

```text
giant cards
large decorative gradients
oversized radii
excessive whitespace
marketing-page visual language
```

Suggested typography:

```text
body                 14 px
secondary metadata   12–13 px
section headings     16–18 px
page heading         22–26 px
```

Monospace should be used for:

```text
field names
document IDs
query syntax
JSON
paths
versions
error codes
```

---

# 61. Responsive behavior

Primary target:

```text
desktop/laptop
```

This is a developer/administration application.

The dashboard should still remain usable on tablet widths.

At narrow widths:

- collapse global sidebar;
- stack Search Lab panels;
- inspector becomes drawer;
- table uses horizontal scrolling.

Do not distort the desktop experience merely to make complex ranking configuration comfortable on a phone.

---

# 62. State management

Avoid one enormous application store.

Use:

| State | Owner |
|---|---|
| Backend/server data | TanStack Query |
| Current route/index | Router |
| Search query parameters | URL |
| Editing form | React Hook Form |
| Dialog/drawer | Local component state |
| Display preferences | localStorage / metadata |
| Server capabilities | Query cache/provider |

Global application state should be minimal.

---

# 63. TanStack Query behavior

Use stable query-key namespaces.

Conceptually:

```text
["indexes"]
["index", indexId]
["documents", indexId, request]
["schema", indexId]
["rankingProfiles", indexId]
["summary", indexId, name]
["metrics", indexId, range]
```

Mutation success invalidates only relevant data.

Example:

```text
document update
    ↓
invalidate document
invalidate current document search/browse
```

Do not indiscriminately invalidate the entire application.

---

# 64. Long-running operations

Some operations may later take meaningful time:

```text
index creation
rebuild
reindex
model loading
configuration apply
```

Do not fake synchronous completion.

If the backend exposes operation IDs, display:

```text
Applying configuration...

Reindexing title
43%

[ View details ]
```

The frontend should support:

```text
pending
running
completed
failed
cancelled
```

where the backend does.

---

# 65. Error model

Portable Search Kernel errors should be rendered structurally.

Example:

```text
Invalid filter

price is a floating-point field and cannot be compared
with the value "cheap".

Path
filter.and[1].value

Code
invalid_filter_value
```

Do not reduce useful errors to:

```text
Something went wrong.
```

---

# 66. Search errors

A failed Search Lab request should generally preserve the previous successful result set.

Display the error above or alongside it.

This lets users correct one parameter without losing context.

---

# 67. Error presentation hierarchy

Use:

### Inline

For field/form validation.

### Page alert

For recoverable page-level request failures.

### Toast

For lightweight confirmations:

```text
Document updated.
```

### Modal

Only for actions requiring explicit intervention.

### Fatal application screen

Only when the application cannot function.

---

# 68. Destructive actions

Index deletion requires explicit confirmation.

Example:

```text
Delete index products?

This permanently removes the index and its stored data.

Type "products" to confirm.

[                            ]

[ Cancel ] [ Delete index ]
```

Potentially destructive configuration changes must show the backend-generated change plan before applying.

---

# 69. Command palette

Keyboard:

```text
Ctrl/Cmd + K
```

Commands:

```text
Go to index...
Search documents...
Open Search Lab
Analyze text
Open ranking profile...
Create index
View Operations
```

Search results should prioritize recently used indexes and current-index commands.

---

# 70. Keyboard interactions

Recommended:

```text
Cmd/Ctrl + K     Command palette
Cmd/Ctrl + Enter Execute Search Lab request
/                Focus principal search field where appropriate
Esc              Close drawer/dialog
```

Avoid creating dozens of undocumented shortcuts.

---

# 71. Loading states

Prefer skeletons matching the actual page geometry.

Avoid full-page spinners after initial application load.

Search Lab should preserve old results while fetching new ones where appropriate.

Tables should preserve column layout while refreshing.

---

# 72. Empty states

Every principal data surface needs a purposeful empty state.

Examples:

### No indexes

```text
No indexes yet.

Create an index to start indexing and searching documents.

[ Create index ]
```

### No documents

```text
This index contains no documents.

[ Add document ]
```

### No search hits

```text
No documents matched this query.

Try removing filters or inspect the resolved query.
```

Where useful:

```text
[ Inspect Query ]
```

---

# 73. Accessibility

Minimum requirements:

- full keyboard navigation;
- visible focus states;
- semantic buttons/links;
- accessible dialogs;
- labels for every form field;
- accessible data tables;
- ARIA live announcement for important async operation state;
- no color-only status indication;
- respect reduced-motion preference.

Tailwind styling must not compromise semantic HTML.

---

# 74. Security

Search Kernel currently has no general authorization model in the project scope.

Therefore:

> The dashboard must not give users a false sense that the daemon is safe to expose publicly.

Default installation should strongly prefer loopback/local access unless authentication is explicitly added.

If `searchkerneld` is configured to listen remotely without authentication, startup and dashboard diagnostics should make this visible.

Do not invent frontend-only authentication.

Authentication has to protect the server.

---

# 75. Secrets

Never expose:

```text
environment variables
credentials
secret headers
filesystem secrets
```

through diagnostics.

"Copy diagnostics" must be explicitly sanitized server-side wherever possible.

---

# 76. Performance goals

The dashboard itself should remain lightweight.

Guidelines:

```text
initial route should not require loading CodeMirror
operations charts should be lazy-loaded
large JSON should be virtualized/truncated sensibly
document tables should not render thousands of DOM rows
```

Code editor and advanced visualization code should be route- or interaction-level lazy imports.

---

# 77. Large documents

Document JSON may be very large.

The drawer must:

- avoid expensive pretty-printing of arbitrarily huge payloads;
- allow collapsed trees or text rendering;
- warn/truncate where necessary;
- allow explicit full loading if the API supports partial projections.

Do not freeze the browser trying to prettify a multi-megabyte vector document.

---

# 78. Large query responses

Search Lab should not accidentally request giant result sets.

Use sensible UI defaults supplied by Search Kernel.

Do not invent separate dashboard semantic defaults.

If the portable API's default limit is 20, the dashboard should use it rather than independently defaulting to 25.

---

# 79. Testing architecture

The frontend test strategy should exploit the existing C-series separation.

Most E2E tests should run against:

```text
real dashboard
    ↓
real searchkerneld HTTP transport
    ↓
RecordingBackend / ScriptedBackend
```

The C3 testkit is explicitly intended to provide non-searching recording/scripted backends for conformance testing.

This makes dashboard testing deterministic without requiring Vespa.

---

# 80. Unit tests

Unit tests should focus on frontend-only behavior:

```text
route construction
formatting
capability interpretation
display-preference persistence
form transformations
error presentation
```

Do not write unit tests that duplicate Search Kernel semantic validation.

---

# 81. Component/integration tests

Examples:

```text
document table respects available fields
unsupported schema action is absent
portable error renders correct path/code
ranking selector shows profiles returned by backend
change-plan dialog renders required actions
```

---

# 82. Playwright tests

Core scenarios:

### Application shell

```text
load dashboard
see indexes
select index
navigate primary sections
reload route
same state restored
```

### Documents

```text
browse
search
open drawer
edit
save
verify refreshed data
delete
```

### Search Lab

```text
enter query
add structured filter
choose ranking profile
execute
verify emitted canonical request
render scripted response
inspect result
```

### Raw request

```text
builder → JSON
JSON → request
both produce semantically equivalent IR
```

### Analyze

```text
submit text
render server-provided linguistic output
```

### Configuration

```text
edit
validate
review change plan
cancel
apply
handle apply failure
```

### Capabilities

```text
unsupported capability hidden
read-only capability visible but non-editable
```

### Errors

```text
structured validation error
backend unavailable
portable error
operation failure
```

---

# 83. Contract assertion

The strongest dashboard test should be:

```text
UI interaction
   ↓
generated TS client
   ↓
HTTP/JSON
   ↓
canonical FlatBuffers
   ↓
RecordingBackend
```

and assert the resulting canonical Search Kernel IR.

Example:

```text
User:
query = headphones
price <= 500
ranking = product-search

Expected canonical IR:
exact frozen fixture
```

This proves the dashboard is expressing Search Kernel semantics rather than merely checking that a network request occurred.

---

# 84. Real-engine tests

Maintain a much smaller Playwright suite against real Vespa integration.

Representative scenario:

```text
create index
insert documents
browse
search
filter
sort
inspect
update
search again
delete
reopen
search
```

Later:

```text
facets
rank profiles
summaries
vectors
```

This is an I-series/end-to-end concern rather than the principal frontend test suite.

---

# 85. Visual regression

Use screenshots for a limited number of stable high-value screens:

```text
global overview
documents
Search Lab
schema
ranking profile
change-plan dialog
operations
```

Do not screenshot-test every tiny component.

---

# 86. Frontend milestone plan

## F0 — Foundation

Deliver:

```text
Vite + React + TypeScript
Tailwind + Tailwind Plus foundation
router
TanStack Query
generated Search Kernel client
application shell
capability bootstrap
error boundary
```

Acceptance:

- production static bundle;
- served by `searchkerneld`;
- no independent semantic API implementation.

---

## F1 — Index navigation

Deliver:

```text
global overview
index list
index selection
index header
index overview
command palette foundation
```

Acceptance:

- all navigation URL-driven;
- reload preserves location;
- unsupported index actions capability-gated.

---

## F2 — Documents

Deliver:

```text
document browser
filtering
columns
document drawer
JSON inspection
write/edit/delete where supported
```

Acceptance:

- uses canonical generated client;
- structured errors rendered;
- no frontend-owned search/filter semantics.

---

## F3 — Search Lab

Deliver:

```text
query builder
filters
search fields
ranking selector
sort
limit
result list
facets where available
result inspection
```

This should be treated as the principal frontend milestone.

---

## F4 — Search debugging

Deliver:

```text
raw JSON
Analyze
Query Inspector
resolved-request inspection
timings where available
copy JSON/curl
```

Acceptance:

- Builder and raw request paths produce equivalent Search Kernel semantics.

---

## F5 — Schema

Deliver:

```text
field browser
field details
schema creation/editing where supported
capability explanations
change-plan flow
```

---

## F6 — Ranking and Summaries

Deliver:

```text
ranking-profile browser/editor
query input configuration
feature inspection
summary browser/editor
Search Lab summary selection
```

---

## F7 — Operations

Deliver:

```text
index health
query performance
indexing/storage
diagnostics
global process operations
```

---

## F8 — Higher-level Search Behavior

As I8 capabilities land:

```text
presets
synonyms
curations
prefix/typo configuration
```

---

## F9 — Production polish

Deliver:

```text
keyboard interactions
command palette completion
display preferences
accessibility audit
large-document handling
visual regression
responsive refinement
diagnostic export
```

---

# 87. Initial release cut

Do not attempt the entire conceptual IA for the first usable dashboard.

A compelling first release is:

```text
Global Overview

Index
├── Overview
├── Documents
├── Search
│   ├── Search Lab
│   ├── Analyze
│   └── Query Inspector
├── Configuration
│   ├── Schema
│   ├── Ranking
│   └── Summaries
└── Operations
    ├── Health
    └── Diagnostics
```

This already provides a serious product.

Hide future sections until functionality exists.

---

# 88. Naming rules

Use **Search Kernel vocabulary** for top-level product concepts.

Good:

```text
Index
Documents
Schema
Ranking
Rank Profile
Summary
Analyze
Search Lab
Query Inspector
```

Avoid leaking implementation names without good reason:

```text
Proton
DocumentDB
Docsum
Config Model
MatchToolsFactory
FastOS
```

These may appear in advanced diagnostics if they materially aid troubleshooting.

---

# 89. Progressive disclosure

The normal UI should answer:

```text
What is my index?
What documents are in it?
Can I search them?
How is search configured?
Why did I get these results?
Is the index healthy?
```

Expert information should be reachable without dominating basic workflows.

Example:

```text
Ranking Profile
    ↓
normal Search Kernel controls
    ↓
Advanced
    ↓
engine representation / diagnostic details
```

---

# 90. Design criterion for every new feature

Before adding a screen, ask:

### 1. Is this a real Search Kernel product concept?

If no, don't add it merely because Vespa exposes it.

### 2. Does this belong to portable search semantics or process administration?

Choose the correct API boundary.

### 3. Can the backend state or capability be authoritative?

If yes, do not duplicate logic in TypeScript.

### 4. Will the user understand the consequence of changing it?

If not, add explanation and impact planning rather than exposing a raw switch.

### 5. Can the feature be inspected/tested immediately?

Where possible, link configuration to Search Lab.

For example:

```text
Ranking profile → Test in Search Lab
Summary → Test in Search Lab
Analyzer → Analyze text
Schema field → Search this field
```

This cross-linking should be a deliberate product characteristic.

---

# 91. Signature Search Kernel workflow

The ideal experience is:

```text
Configure
    ↓
Test
    ↓
Inspect
    ↓
Understand
    ↓
Adjust
```

Example:

```text
Edit ranking profile
      ↓
Test "wireless headphones"
      ↓
Compare hits
      ↓
Inspect ranking features
      ↓
Understand why sku-42 is #7
      ↓
Adjust ranking
```

This is where the dashboard can become more than a configuration GUI.

---

# 92. Search Kernel's unique dashboard advantage

The dashboard should make the engine's semantic pipeline visible:

```text
what I expressed
      ↓
what Search Kernel resolved
      ↓
what the search engine executed
      ↓
what matched
      ↓
how ranking behaved
      ↓
what result representation was returned
```

That aligns directly with Search Kernel's architectural separation between Portable API, Search Kernel IR, backend lowering, and the Vespa runtime.

This should become the product's defining UX principle.

---

# 93. Final architecture

```text
                         Browser

              React + TypeScript + Tailwind Plus
                            │
                            │
                 Search Kernel generated
                    TypeScript client
                            │
                            ▼
                    HttpJsonTransport
                            │
                            ▼
                      searchkerneld
                            │
                generated JSON codec
                            │
                            ▼
                canonical FlatBuffers
                            │
                            ▼
                  shared dispatcher
                            │
                            ▼
                  searchkernel-core
                            │
                    Search Kernel IR
                            │
                            ▼
                   integrated backend
                            │
                            ▼
                         Vespa
```

Side channel:

```text
Dashboard
    │
    └── narrow admin/diagnostics API
            │
            ├── process metrics
            ├── versions
            ├── disk/RSS
            └── diagnostics
```

The admin side channel must never become an alternative implementation of portable search semantics.

---

# 94. Final product structure

The stable product navigation should be:

```text
Search Kernel
│
├── Overview
├── Indexes
│   │
│   └── Index
│       │
│       ├── Overview
│       │
│       ├── Data
│       │   └── Documents
│       │
│       ├── Search
│       │   ├── Search Lab
│       │   ├── Analyze
│       │   └── Query Inspector
│       │
│       ├── Configuration
│       │   ├── Schema
│       │   ├── Ranking
│       │   ├── Summaries
│       │   ├── Search Behavior
│       │   └── Assets
│       │
│       └── Operations
│           ├── Health
│           ├── Query Performance
│           ├── Indexing & Storage
│           └── Diagnostics
│
├── Operations
└── Settings
```

This hierarchy should be treated as the baseline frontend product architecture.

It is intentionally broad enough to accommodate the meaningful local Vespa capabilities that Search Kernel intends to expose, while excluding the distributed Vespa control-plane concepts that Search Kernel deliberately removes.

The visual experience should remain simpler than the conceptual hierarchy.

The user should feel like they are using a polished Algolia-style search workbench.

The architecture underneath should remain unmistakably Search Kernel.