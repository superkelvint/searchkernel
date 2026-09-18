# Portable API schemas

This directory contains the language-neutral FlatBuffers schemas used at the
boundary between callers and Search Kernel.

## Start reading here

1. Read `searchkernel.fbs` for the domain model: indexes, documents, values,
   queries, filters, and results.
2. Read `searchkernel_request.fbs` and `searchkernel_response.fbs` for the
   neutral canonical request/response envelopes used by dispatchers.
3. Follow the Rust conversion code in
   `core/crates/searchkernel-ir/src/api_to_ir.rs` and
   `core/crates/searchkernel-ir/src/ir_to_api.rs`.

## Concepts to know

- FlatBuffers tables are borrowed views over bytes. `searchkernel-ir` converts
  them into owned Rust values before backend execution.
- Optional fields preserve presence. Missing and explicitly supplied default
  values are not automatically treated as the same thing.
- Union discriminators must agree with their payload tables. Malformed or
  incomplete unions are rejected at the boundary.
- The schemas describe the portable contract; they do not implement search.

Generated Rust protocol code belongs under
`core/crates/searchkernel-protocol/` and should not be edited by hand.

## JSON tooling

The JSON transport uses the official FlatBuffers C++ parser and text generator
from FlatBuffers 25.12.19. Install that release on the developer system. Set
`FLATBUFFERS_ROOT` when its headers and library are outside `/usr/local`; the
headers and library must come from the same installation. Put the matching
`flatc` on `PATH`, or set `FLATC_PATH`, when regenerating the committed BFBS
artifacts.
