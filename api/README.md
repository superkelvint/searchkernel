# Portable API schemas

This directory contains the language-neutral FlatBuffers schemas used at the
portable boundary between non-Rust callers, `searchkerneld`, and Search Kernel.
The first-party Rust API is embedded directly and does not serialize requests
through these schemas merely to reach the local engine.

## Start reading here

1. Read `searchkernel.fbs` for the domain model: indexes, documents, values,
   queries, filters, and results.
2. Read `searchkernel_request.fbs` and `searchkernel_response.fbs` for the
   neutral canonical request/response envelopes used by dispatchers.
3. Follow the Rust conversion code in
   `core/crates/searchkernel-ir/src/api_to_ir/mod.rs` and
   `core/crates/searchkernel-ir/src/ir_to_api/mod.rs`.

## Concepts to know

- FlatBuffers tables are borrowed views over bytes. The portable dispatch path
  converts them into owned Rust values before backend execution. The direct
  Rust API constructs resolver inputs in Rust and converges at the same IR
  boundary without a FlatBuffers round trip.
- Optional fields preserve presence. Missing and explicitly supplied default
  values are not automatically treated as the same thing.
- Union discriminators must agree with their payload tables. Malformed or
  incomplete unions are rejected at the boundary.
- The schemas describe the portable contract; they do not implement search.

Generated Rust protocol code belongs under
`core/crates/searchkernel-protocol/` and should not be edited by hand.

## JSON tooling

The JSON transport uses the official FlatBuffers C++ parser and text generator
from FlatBuffers 25.12.19. The Cargo build obtains the headers, static library,
and `flatc` from one pinned vendored source and runs a real parser link probe
before exposing them to the protocol crate. Standalone regeneration scripts
may use `flatc` from `PATH` or `FLATC_PATH`, but they verify the exact pinned
version.

The native-build regression verifier is `bash tools/test-native-build-hardening.sh`.
