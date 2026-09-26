# `searchkernel-protocol`

This crate contains the Rust representation generated from the portable
FlatBuffers schemas in `api/`.

## Start reading here

1. Read `src/lib.rs` to see the generated module tree.
2. Read the schema files in `api/` for the intended wire shape.
3. Read `core/crates/searchkernel-ir/src/api_to_ir/mod.rs` to see how generated
   views are validated and made owned.

## Key concepts

- These types are wire views, not the product API and not backend IR.
- Their lifetimes are tied to the FlatBuffers byte buffer they borrow.
- The direct Rust API does not use these generated views to reach the in-process
  engine; they are for the portable FlatBuffers/JSON/gRPC path.
- Generated files are reproducible outputs. Change the schema and generation
  tooling, never an individual generated file.
- `json` is only the generic standard FlatBuffers JSON ↔ BFBS boundary; it does
  not define a Search Kernel JSON dialect or semantic mapping layer.
- Canonical JSON spells schema `long`/`ulong` scalars as quoted decimal
  strings. The official FlatBuffers parser accepts that standard scalar
  spelling, while FlatBuffers 25.12.19 `GenText` emits bare JSON numbers.
  The native bridge therefore performs a BFBS-driven lexical normalization
  around the official parser/printer: it rejects bare 64-bit input numbers and
  quotes only schema-typed 64-bit printer output. It contains no Search Kernel
  field-name mappings and does not bypass the canonical FlatBuffer/IR path.

The native JSON bridge links against the repository's vendored FlatBuffers
25.12.19 build. Its compiler, target, libc, C++ runtime, CMake configuration,
and native flags are recorded in a fingerprint beside the CMake output. A
fingerprint mismatch or failed parser link probe invalidates only that
FlatBuffers output and retries one clean native rebuild automatically.
The local `flatbuffers-build` fork is a normal dependency solely so Cargo can
pass its `links` metadata to this crate's build script; the crate does not use
that helper at runtime. This avoids scanning a shared target directory for a
possibly unrelated archive.
