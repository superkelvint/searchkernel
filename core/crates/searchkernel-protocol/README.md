# `searchkernel-protocol`

This crate contains the Rust representation generated from the portable
FlatBuffers schemas in `api/`.

## Start reading here

1. Read `src/lib.rs` to see the generated module tree.
2. Read the schema files in `api/` for the intended wire shape.
3. Read `searchkernel-ir/src/api_to_ir.rs` to see how generated views are
   validated and made owned.

## Key concepts

- These types are wire views, not the product API and not backend IR.
- Their lifetimes are tied to the FlatBuffers byte buffer they borrow.
- Generated files are reproducible outputs. Change the schema and generation
  tooling, never an individual generated file.
- `json` is only the generic standard FlatBuffers JSON ↔ BFBS boundary; it does
  not define a Search Kernel JSON dialect or semantic mapping layer.
