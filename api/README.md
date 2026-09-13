# Portable API schemas

This directory contains the language-neutral FlatBuffers schemas used at the
boundary between callers and Search Kernel.

## Start reading here

1. Read `searchkernel.fbs` for the domain model: indexes, documents, values,
   queries, filters, and results.
2. Read `searchkernel_ffi_request.fbs` and `searchkernel_ffi_response.fbs` for
   the small request/response envelopes used by the native ABI.
3. Follow the Rust conversion code in
   `crates/searchkernel-core/src/api_to_ir.rs` and
   `crates/searchkernel-core/src/ir_to_api.rs`.

## Concepts to know

- FlatBuffers tables are borrowed views over bytes. The core crate converts
  them into owned Rust values before backend execution.
- Optional fields preserve presence. Missing and explicitly supplied default
  values are not automatically treated as the same thing.
- Union discriminators must agree with their payload tables. Malformed or
  incomplete unions are rejected at the boundary.
- The schemas describe the portable contract; they do not implement search.

Generated Rust code belongs under the API crate and should not be edited by
hand.
