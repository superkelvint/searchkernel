# `searchkernel-ir`

This crate is the semantic center of Search Kernel. It resolves both direct
Rust API inputs and portable FlatBuffers requests into validated, owned,
backend-independent IR, and encodes owned results back into the portable
contract when a portable response is required.

The direct Rust API does not make a FlatBuffers round trip. Its builders create
resolver inputs such as `SearchIntent`, which use the same semantic resolution
rules as the portable API path.

## Start reading here

1. `src/model.rs` defines the owned IR and result values.
2. `src/api_to_ir/mod.rs` and `src/api_to_ir/resolve.rs` perform parsing,
   validation, defaults, and normalization for portable and Rust resolver
   inputs.
3. `src/backend.rs` defines the execution surface a backend receives.
4. `src/ir_to_api/mod.rs` encodes errors, requests, and results for the
   portable contract.
5. `tests/semantics.rs` is the behavioral specification for edge cases.

## Key concepts

- Portable FlatBuffers views are temporary; IR owns strings, collections, and
  values. Direct Rust resolver inputs are also normalized into the same owned
  IR.
- This crate owns portable defaults and validation. Backends receive explicit
  meaning and must not reinterpret wire omissions.
- Error codes and paths are part of the portable behavior.
- Boolean query/filter flattening is canonical IR normalization, not backend
  execution.
- The backend trait is an execution interface, not a second search engine.

If a change appears to require native-engine types here, the boundary is
probably in the wrong place.
