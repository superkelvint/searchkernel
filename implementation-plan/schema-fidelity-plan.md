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

**Tests only. No production fixes.**

## Canonical control

-  Build one comprehensive canonical `IndexDefinition` fixture.
-  Resolve it through the real FlatBuffers → IR path.
-  Prove `IndexDefinitionIr -> FlatBuffers -> IndexDefinitionIr` is lossless.
-  Keep the existing `describe_preserves_configured_rank_profiles` test green.

## IndexDefinitionIr coverage

The comprehensive fixture must cover every current top-level field:

-  `name`
-  `fields`
-  `default_search_fields`
-  `default_ranking`
-  `default_prefix_policy`
-  `default_typo_policy`
-  `properties`
-  `document_type`
-  `schema_inherits`
-  `document_inherits`
-  `structs`
-  `fieldsets`
-  `schema_fields`
-  `imported_fields`
-  `document_summaries`
-  `rank_profiles`
-  `constants`
-  `onnx_models`

## DataTypeIr coverage

The fixture must contain fields exercising:

-  `Primitive(String)`
-  `Primitive(Bool)`
-  `Primitive(Byte)`
-  `Primitive(Int32)`
-  `Primitive(Int64)`
-  `Primitive(Float32)`
-  `Primitive(Float64)`
-  `Primitive(Raw)`
-  `Tensor`
-  `Array`
-  `Map`
-  `WeightedSet`
-  `Struct`
-  `StructReference`
-  `Predicate`
-  `Position`
-  `Reference`

## FieldIr coverage

Exercise every field:

-  `name`
-  `data_type`
-  `analyzer`
-  `capabilities`
-  `matching`
-  `index`
-  `attribute`
-  `summary`
-  `indexing_script`
-  `properties`
-  `struct_fields`

## FieldCapabilities coverage

-  `stored`
-  `searchable`
-  `filterable`
-  `sortable`
-  `groupable`

## MatchingConfigIr coverage

-  `mode`
-  `options`

## IndexConfigIr coverage

-  `enabled`
-  `hnsw`
-  `options`

### HnswIndexConfigIr

-  `max_links_per_node`
-  `neighbors_to_explore_at_insert`
-  `options`

## AttributeConfigIr coverage

-  `enabled`
-  `fast_search`
-  `fast_access`
-  `distance_metric`
-  `options`

## SummaryConfigIr coverage

-  `enabled`
-  `options`

## StructFieldConfigIr coverage

-  `path`
-  `indexing_script`
-  `matching`
-  `index`
-  `attribute`
-  `summary`
-  `rank`
-  `query_commands`
-  `properties`

## SearchFieldIr coverage

-  `field`
-  `weight`

## NamedStructDefinitionIr coverage

-  `name`
-  `fields`
-  `properties`

## FieldSetDefinitionIr coverage

-  `name`
-  `fields`
-  `matching`
-  `query_commands`
-  `properties`

## ImportedFieldDefinitionIr coverage

-  `reference_field`
-  `target_field`
-  `alias`
-  `properties`

## DocumentSummaryDefinitionIr coverage

-  `name`
-  `inherits`
-  `fields`
-  `from_disk`
-  `omit_summary_features`
-  `properties`

### DocumentSummaryFieldDefinitionIr

-  `name`
-  `source`
-  `data_type`
-  `struct_fields`
-  `properties`

## ConstantDefinitionIr coverage

-  `name`
-  `data_type`
-  `source`
-  `properties`
-  literal source
-  file source

## OnnxModelDefinitionIr coverage

-  `name`
-  `file`
-  `inputs`
-  `outputs`
-  `gpu_device`
-  `intraop_threads`
-  `interop_threads`
-  `properties`

------

# Rank-profile fixture

Use multiple profiles because mutually exclusive phase-count fields cannot all coexist legally.

Required profiles:

-  `complete_profile`
-  `alternate_counts_profile`
-  `sparse_profile`

## RankProfileDefinitionIr coverage

Collectively exercise:

-  `name`
-  `inherits`
-  `inputs`
-  `functions`
-  `first_phase`
-  `second_phase`
-  `global_phase`
-  `rank_features`
-  `match_features`
-  `summary_features`
-  `sort_features`
-  `constants`
-  `onnx_models`
-  `rank_properties`
-  `strict`
-  `properties`

## RankProfileInputIr coverage

-  `name`
-  `data_type`
-  `default_value`
-  scalar input/default
-  tensor input/default

## RankFunctionDefinitionIr coverage

-  `name`
-  `arguments`
-  `expression`
-  `inline = Some(true)`
-  `inline = Some(false)`
-  `inline = None`

## RankPhaseDefinitionIr coverage

Collectively exercise:

-  `expression`
-  `rerank_count`
-  `rank_score_drop_limit`
-  `properties`
-  `total_rerank_count`
-  `keep_rank_count`
-  `total_keep_rank_count`

Also test:

-  absent `first_phase`
-  absent `second_phase`
-  absent `global_phase`

------

# Required focused regressions

Add focused tests for:

-  rank-profile inheritance
-  rank-profile inputs and default values
-  rank-profile functions
-  complete rank-phase state
-  absent rank phases
-  all rank feature lists
-  rank-profile constants
-  rank-profile ONNX models
-  rank properties / strict / generic properties
-  all document data types
-  complete field configuration
-  struct-field configuration
-  default search fields and weights
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

Finally:

-  Add `describe_round_trips_complete_index_definition_without_semantic_loss`.
-  Compare full `IndexDefinitionIr` equality.
-  Do not normalize away differences.
-  Do not ignore the test.
-  Do not encode current lossy behavior as expected behavior.

### Expected state after Phase 1

-  Canonical IR ↔ FlatBuffers control passes.
-  Existing narrow describe regression passes.
-  Already-supported positive controls pass.
-  New fidelity regressions fail only because the Rust describe boundary loses canonical state.
-  No production implementation changed.

------

# Phase 2 — Make the public Rust schema representation lossless

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