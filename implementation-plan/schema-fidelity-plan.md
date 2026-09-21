# Rust Schema Fidelity Plan

## Problem

The canonical schema is represented by `IndexDefinitionIr`, but the public Rust schema types used by `Index::describe()` currently represent only a subset of that state.

Current path:

```text
IndexDefinitionIr
    -> definition_from_ir()
    -> public Rust IndexDefinition
    -> Index::describe()
```

is lossy.

The target invariant is:

```text
IndexDefinitionIr
    -> public Rust representation
    -> IndexDefinitionIr

== original semantic state
```

This plan fixes the problem in explicit stages.

------

# Phase 1 — Freeze regressions

**Tests-only regression freeze. Production fidelity remains open.**

The canonical fixture and the independent FlatBuffers ↔ IR control are
implemented. The regression tests from this phase do not establish public Rust
fidelity when the conversion can retain or return an opaque canonical IR
sidecar. Public-schema implementation remains open in Phases 2–5.

## Canonical control

-  [x] Build one comprehensive canonical `IndexDefinition` fixture.
-  [x] Resolve it through the real FlatBuffers → IR path.
-  [x] Prove `IndexDefinitionIr -> FlatBuffers -> IndexDefinitionIr` is lossless.
-  [x] Keep the existing `describe_preserves_configured_rank_profiles` test green.

## IndexDefinitionIr coverage

The comprehensive fixture must cover every current top-level field:

-  [x] `name`
-  [x] `fields`
-  [x] `default_search_fields`
-  [x] `default_ranking`
-  [x] `default_prefix_policy`
-  [x] `default_typo_policy`
-  [x] `properties`
-  [x] `document_type`
-  [x] `schema_inherits`
-  [x] `document_inherits`
-  [x] `structs`
-  [x] `fieldsets`
-  [x] `schema_fields`
-  [x] `imported_fields`
-  [x] `document_summaries`
-  [x] `rank_profiles`
-  [x] `constants`
-  [x] `onnx_models`

## DataTypeIr coverage

The fixture must contain fields exercising:

-  [x] `Primitive(String)`
-  [x] `Primitive(Bool)`
-  [x] `Primitive(Byte)`
-  [x] `Primitive(Int32)`
-  [x] `Primitive(Int64)`
-  [x] `Primitive(Float32)`
-  [x] `Primitive(Float64)`
-  [x] `Primitive(Raw)`
-  [x] `Tensor`
-  [x] `Array`
-  [x] `Map`
-  [x] `WeightedSet`
-  [x] `Struct`
-  [x] `StructReference`
-  [x] `Predicate`
-  [x] `Position`
-  [x] `Reference`

## FieldIr coverage

Exercise every field:

-  [x] `name`
-  [x] `data_type`
-  [x] `analyzer`
-  [x] `capabilities`
-  [x] `matching`
-  [x] `index`
-  [x] `attribute`
-  [x] `summary`
-  [x] `indexing_script`
-  [x] `properties`
-  [x] `struct_fields`

## FieldCapabilities coverage

-  [x] `stored`
-  [x] `searchable`
-  [x] `filterable`
-  [x] `sortable`
-  [x] `groupable`

## MatchingConfigIr coverage

-  [x] `mode`
-  [x] `options`

## IndexConfigIr coverage

-  [x] `enabled`
-  [x] `hnsw`
-  [x] `options`

### HnswIndexConfigIr

-  [x] `max_links_per_node`
-  [x] `neighbors_to_explore_at_insert`
-  [x] `options`

## AttributeConfigIr coverage

-  [x] `enabled`
-  [x] `fast_search`
-  [x] `fast_access`
-  [x] `distance_metric`
-  [x] `options`

## SummaryConfigIr coverage

-  [x] `enabled`
-  [x] `options`

## StructFieldConfigIr coverage

-  [x] `path`
-  [x] `indexing_script`
-  [x] `matching`
-  [x] `index`
-  [x] `attribute`
-  [x] `summary`
-  [x] `rank`
-  [x] `query_commands`
-  [x] `properties`

## SearchFieldIr coverage

-  [x] `field`
-  [x] `weight`

## NamedStructDefinitionIr coverage

-  [x] `name`
-  [x] `fields`
-  [x] `properties`

## FieldSetDefinitionIr coverage

-  [x] `name`
-  [x] `fields`
-  [x] `matching`
-  [x] `query_commands`
-  [x] `properties`

## ImportedFieldDefinitionIr coverage

-  [x] `reference_field`
-  [x] `target_field`
-  [x] `alias`
-  [x] `properties`

## DocumentSummaryDefinitionIr coverage

-  [x] `name`
-  [x] `inherits`
-  [x] `fields`
-  [x] `from_disk`
-  [x] `omit_summary_features`
-  [x] `properties`

### DocumentSummaryFieldDefinitionIr

-  [x] `name`
-  [x] `source`
-  [x] `data_type`
-  [x] `struct_fields`
-  [x] `properties`

## ConstantDefinitionIr coverage

-  [x] `name`
-  [x] `data_type`
-  [x] `source`
-  [x] `properties`
-  [x] literal source
-  [x] file source

## OnnxModelDefinitionIr coverage

-  [x] `name`
-  [x] `file`
-  [x] `inputs`
-  [x] `outputs`
-  [x] `gpu_device`
-  [x] `intraop_threads`
-  [x] `interop_threads`
-  [x] `properties`

------

# Rank-profile fixture

Use multiple profiles because mutually exclusive phase-count fields cannot all coexist legally.

Required profiles:

-  [x] `complete_profile`
-  [x] `alternate_counts_profile`
-  [x] `sparse_profile`

## RankProfileDefinitionIr coverage

Collectively exercise:

-  [x] `name`
-  [x] `inherits`
-  [x] `inputs`
-  [x] `functions`
-  [x] `first_phase`
-  [x] `second_phase`
-  [x] `global_phase`
-  [x] `rank_features`
-  [x] `match_features`
-  [x] `summary_features`
-  [x] `sort_features`
-  [x] `constants`
-  [x] `onnx_models`
-  [x] `rank_properties`
-  [x] `strict`
-  [x] `properties`

## RankProfileInputIr coverage

-  [x] `name`
-  [x] `data_type`
-  [x] `default_value`
-  [x] scalar input/default
-  [x] tensor input/default

## RankFunctionDefinitionIr coverage

-  [x] `name`
-  [x] `arguments`
-  [x] `expression`
-  [x] `inline = Some(true)`
-  [x] `inline = Some(false)`
-  [x] `inline = None`

## RankPhaseDefinitionIr coverage

Collectively exercise:

-  [x] `expression`
-  [x] `rerank_count`
-  [x] `rank_score_drop_limit`
-  [x] `properties`
-  [x] `total_rerank_count`
-  [x] `keep_rank_count`
-  [x] `total_keep_rank_count`

Also test:

-  [x] absent `first_phase`
-  [x] absent `second_phase`
-  [x] absent `global_phase`

------

# Required focused regressions

Add focused tests for:

-  [x] rank-profile inheritance
-  [x] rank-profile inputs and default values
-  [x] rank-profile functions
-  [x] complete rank-phase state
-  [x] absent rank phases
-  [x] all rank feature lists
-  [x] rank-profile constants
-  [x] rank-profile ONNX models
-  [x] rank properties / strict / generic properties
-  [x] all document data types
-  [x] complete field configuration
-  [x] struct-field configuration
-  [x] default search fields and weights
-  [x] schema properties
-  [x] document type
-  [x] schema inheritance
-  [x] document inheritance
-  [x] named structs
-  [x] fieldsets
-  [x] schema-only fields
-  [x] imported fields
-  [x] document summaries
-  [x] schema constants
-  [x] schema ONNX models

Finally:

-  [x] Add `describe_round_trips_complete_index_definition_without_semantic_loss`.
-  [x] Compare full `IndexDefinitionIr` equality.
-  [x] Do not normalize away differences.
-  [x] Do not ignore the test.
-  [x] Do not encode current lossy behavior as expected behavior.

### Expected implementation state

-  [x] Canonical IR ↔ FlatBuffers control passes.
-  [x] Existing narrow describe regression passes.
-  [x] Already-supported positive controls pass.
-  [ ] New fidelity regressions pass because the public Rust describe boundary
       itself preserves canonical state; sidecar-backed round trips do not
       count as evidence.
-  [ ] Production changes complete the schema-fidelity conversion path.

------

# Phase 2 — Make the public Rust schema representation lossless

Implementation status: open. The current public representation remains a
lossy ergonomic projection with an opaque canonical-IR sidecar. Exposing that
sidecar or returning it from conversion is not public-schema fidelity evidence.
This phase remains open until `Index::describe()` returns one truthful public
representation that directly exposes the canonical concepts below.

-  [ ] `IndexDefinition` exposes complete canonical top-level schema state.
-  [ ] `Field` exposes complete canonical field/data-type state.
-  [ ] `RankProfile` exposes complete canonical rank-profile and phase state.
-  [ ] Public described values retain absence/presence semantics.

Do not solve this by adding special cases inside `describe()`.

The public Rust representation must be capable of preserving every canonical schema concept.

## Index definition

Extend or replace the current public representation so it can represent:

-  schema properties
-  document type
-  schema inheritance
-  document inheritance
-  named structs
-  fieldsets
-  schema-only fields
-  imported fields
-  document summaries
-  schema constants
-  schema ONNX models
-  complete field definitions
-  complete canonical data types

## Rank profiles

The public Rust rank-profile representation must support:

-  inheritance
-  complete inputs including defaults
-  functions
-  optional first phase
-  complete first-phase controls
-  optional second phase
-  complete second-phase controls
-  optional global phase
-  complete global-phase controls
-  rank features
-  match features
-  summary features
-  sort features
-  constants
-  ONNX models
-  rank properties
-  strict
-  generic properties

Keep ergonomic constructors/builders where useful.

Do not require ordinary users to populate every advanced field.

------

# Phase 3 — Make Rust → IR conversion lossless

Implementation status: open. `definition_to_ir()` currently has a hidden
canonical-state escape hatch, so full equality after describe does not prove
that conversion reads the public Rust representation.

-  [ ] `definition_to_ir()` converts the complete public representation without
       an immediate hidden-sidecar return.
-  [ ] Cloned fields and rank profiles convert from their own public state.
-  [ ] Rich public mutations update one authoritative public representation
       while preserving untouched members.

Update the production Rust schema conversion path.

Required invariant:

```text
public Rust schema
    -> IndexDefinitionIr
```

must preserve all represented state.

Update:

-  `definition_to_ir()`
-  rank-profile conversion
-  field conversion
-  nested schema conversion
-  constants
-  ONNX models
-  document summaries
-  inheritance
-  properties/options
-  optional/presence semantics

Remove any construction of artificial canonical defaults where the public type can now preserve the actual value.

------

# Phase 4 — Make IR → Rust conversion lossless

Implementation status: open. `definition_from_ir()` still materializes a
narrow public projection and retains canonical state separately; the public
projection must be made complete before this phase can be checked off.

-  [ ] `definition_from_ir()` retains complete field and rank-profile state on
       the returned public values.
-  [ ] Optional rank phases are represented without manufacturing an empty
       first phase.
-  [ ] Top-level canonical state is retained and publicly observable through
       the public representation itself.

Update:

```text
definition_from_ir()
```

and all nested conversions.

Required invariant:

```text
IndexDefinitionIr
    -> public Rust schema
```

must preserve every canonical field.

No silent dropping.

No substitution of empty collections for meaningful values.

No creation of an empty first phase when canonical first phase is absent.

No fake tensor defaults.

------

# Phase 5 — Make describe round-trip lossless

Implementation status: open. The describe round trip remains unproven until
the public representation itself is lossless and persistence uses that same
authoritative state.

-  [ ] `Index::describe()` exposes the complete canonical state through the
       public Rust representation.
-  [ ] Full canonical equality passes after describe, clone/reuse, and rich
       mutation paths without a sidecar bypass.
-  [ ] Rich described definitions use canonical FlatBuffers provenance for
       portable persistence.

After Phases 2–4:

```text
Index::describe()
```

must return a representation capable of preserving the complete canonical schema state held by the index context.

Make every Phase 1 regression green.

Required final invariant:

```text
original IndexDefinitionIr
    -> Index
    -> describe()
    -> IndexDefinitionIr

== original IndexDefinitionIr
```

------

# Phase 6 — Audit other Rust/public semantic boundaries

Search the repository for any other conversions where a rich canonical IR type is converted into a narrower Rust-facing type.

Audit:

-  search request inspection
-  query inspection
-  search results
-  grouping results
-  ranking results/features
-  analyze results
-  document summaries
-  mutation/update descriptions if present

For every boundary:

-  identify whether it is intentionally ergonomic input-only;
-  identify whether it is also used for introspection/output;
-  prohibit silent semantic loss on output/introspection paths.

------

# Phase 7 — Backend support is separate

Do not confuse schema representation with Vespa executability.

After schema fidelity is fixed, audit `searchkernel-ir-adapter` separately.

For every represented rank-profile capability, classify it as:

```text
REPRESENTED
SEMANTIC
EXECUTABLE
ERGONOMIC
```

Backend lowering may still return `Unsupported` for functionality not yet implemented.

It must not be removed from `describe()` merely because it is not executable.

Audit backend execution support for:

-  rank inheritance
-  rank functions
-  all first-phase controls
-  all second-phase controls
-  global phase
-  rank features
-  summary features
-  sort features
-  constants
-  ONNX models
-  rank properties
-  strict
-  rank-profile properties
-  complete query-input/default semantics

Each executable capability requires its own Vespa-backed verification.

------

# Completion criteria

This work is complete only when:

-  canonical FlatBuffers ↔ IR round-trip remains lossless;
-  canonical IR ↔ public Rust schema round-trip is lossless;
-  `Index::describe()` is lossless;
-  all Phase 1 regressions pass;
-  Rust ergonomics remain usable for ordinary schemas;
-  unsupported backend execution remains explicit `Unsupported`, never silent semantic loss;
-  documentation no longer describes lossy Rust introspection as an accepted limitation.
