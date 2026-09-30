# M6 — Vespa schema/configuration

## Goal

Remove the dependency on pre-generated Java configuration for the supported low-level `searchkernel-vespa-engine` schema subset.

M6 owns only the low-level embedded-Vespa schema/configuration model. It does not define the C-series product schema.

Required path:

```text
searchkernel-vespa-engine Schema
        ↓
ConfigCompiler
        ↓
fresh self-describing configuration directory
        ↓
Engine::open
        ↓
real DocumentDB / Proton
```

## Supported subset

M6 v1 supports exactly:

```text
FieldType::Text
FieldType::String
FieldType::Integer
FieldType::Float
FieldType::Boolean
```

with capabilities:

```text
index
attribute
summary
```

Rules:

- `Text` must be indexed; it may additionally be an attribute and/or summary field.
- `String` is exact/keyword semantics; it must be an attribute, may be summary, and is not text-indexed in M6.
- `Integer`, `Float`, and `Boolean` must be attributes, may be summary, and are not text-indexed in M6.
- Document-type and field names match `[A-Za-z_][A-Za-z0-9_]*`.
- Field names are unique within a schema.
- Arrays, weighted sets, tensors, references, structs, maps, aliases, fieldsets, custom stemming/match modes, and custom ranking are outside M6.

## Frozen public API

The compile-only fixture at:

```text
core/crates/searchkernel-vespa-engine/examples/api-contract-m6.rs
```

is normative.

The implementation provides the frozen low-level API:

```text
FieldType
Field
Schema
ConfigCompiler
```

If prose and the compile-only fixture disagree, the fixture wins.

## Document type is engine state

`Schema::new(document_type)` is not decorative metadata.

The compiled configuration directory MUST preserve the document type and the complete supported field model so a later independent `Engine::open(config_dir)` can construct the correct native document model without receiving the Rust `Schema` object again.

M6 freezes one Search Kernel-owned private manifest for this purpose:

```text
searchkernel-schema.json
```

Its canonical JSON shape is:

```json
{
  "version": 1,
  "document_type": "products",
  "fields": [
    {
      "name": "title",
      "type": "text",
      "index": true,
      "attribute": false,
      "summary": true
    }
  ]
}
```

Rules:

- `version` is exactly `1`.
- `document_type` is exactly `Schema::document_type`.
- `fields` contains every M6 field exactly once, in schema insertion order.
- `type` is one of `text`, `string`, `integer`, `float`, `boolean`.
- Capability booleans are explicit.
- The manifest is private implementation plumbing, not a public schema language.
- Production code must not parse `.sd` files.

The core/native/safe engine bootstrap may consume this manifest on either side of the private Rust/native boundary, but the resulting `DocumentDB` must use the compiled document type and fields. The document type must not remain hard-coded to the historical `typea` fixture.

## Oracle

The pinned Vespa Java config-model is authoritative for the selected low-level schema fixtures.

For each scenario the verifier constructs an equivalent `.sd` file and runs the pinned:

```text
ApplicationBuilder
    ↓
DerivedConfiguration
    ├── IndexSchema
    ├── AttributeFields
    └── Summaries
```

The Java oracle emits a canonical semantic projection containing:

```text
schema/document type
indexed fields:
    name
    datatype
    collection type
    prefix
    positions
    interleaved features
attributes:
    name
    datatype
    collection type
default summary fields:
    name
    command
    source
```

The Rust verifier independently reads the generated configuration directory and projects the same semantics.

It deliberately does not byte-compare Java-generated files or require Search Kernel to reproduce unrelated Java config-model implementation details.

## Required generated files

Every successful compile must create, at minimum:

```text
searchkernel-schema.json
indexschema.cfg
attributes.cfg
summary.cfg
```

plus every additional file required by the embedded native engine.

The directory must be freshly generated from `Schema`; production must not copy M4/M5 fixtures or Java oracle output.

## Document-type independence fixture

M6 contains two scenarios with identical field definitions and different document types:

```text
typea
products
```

The generated directories MUST therefore differ in their document-model identity even though the Vespa index/attribute/summary projections are otherwise equivalent.

Both configurations are opened in independent processes and exercised through the real engine.

This specifically rejects an implementation that validates `Schema::document_type` and then discards it.

## Behavioral acceptance

Semantic parity is necessary but not sufficient.

Each Rust-generated directory is opened using the real `Engine`.

Selected cases exercise existing verified paths:

```text
Text:
    compile
    open
    put(Document)
    term search

Integer:
    compile
    open
    put integer attribute
    integer search

Alternate document type:
    compile products schema
    independent Engine::open
    put(Document)
    term search
```

The alternate-document-type smoke is normative. A native adapter that still hard-codes `typea` does not satisfy M6.

## Java is verification only

Production M6 code must not:

```text
spawn Java
embed a JVM
invoke Maven
invoke config-model
read Java oracle output
```

## Output-directory semantics

`ConfigCompiler::compile_to` accepts an output path that is absent or exists and is empty.

It MUST reject a non-empty output directory rather than merge with stale files.

Compilation must not write outside the requested directory except for a same-filesystem temporary sibling used for atomic installation.

## Forbidden shortcuts

M6 acceptance rejects:

- Java config-model at runtime;
- copying an earlier fixture as production output;
- `.sd` parsing in production;
- reading `reference/vespa/m6/expected` from production;
- hard-coded scenario IDs or field values;
- a dummy `searchkernel-schema.json` that is not the actual compiled schema;
- a native document type hard-coded to `typea`;
- high-level C-series product semantics in `searchkernel-vespa-engine`;
- importing `searchkernel-ir` into the M-series.

## Definition of done

M6 is complete only when:

```text
M0–M5 remain green
M6 API fixture compiles
fmt/clippy/tests pass
Rust semantic config == frozen Java oracle
Rust semantic config == live pinned Java oracle
searchkernel-schema.json exactly represents each Schema
typea/products identical-field schemas retain distinct document identity
all supported schemas open real DocumentDB
text smoke passes through real M3/M4/Matcher
integer smoke passes through real M2 path
products text smoke passes through the real engine
no hard-coded native typea fixture remains
no Java runtime dependency exists
checked-in M6 verifier and reference files remain unchanged during implementation
```
