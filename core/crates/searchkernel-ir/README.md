# `searchkernel-ir`

This crate is the semantic center of Search Kernel. It turns portable
FlatBuffers requests into validated, owned, backend-independent IR and encodes
owned results back into the portable contract.

## Start reading here

1. `src/model.rs` defines the owned IR and result values.
2. `src/api_to_ir.rs` performs parsing, validation, defaults, and
   normalization.
3. `src/backend.rs` defines the execution surface a backend receives.
4. `src/ir_to_api.rs` encodes errors, requests, and results.
5. `tests/semantics.rs` is the behavioral specification for edge cases.

## Key concepts

- FlatBuffers views are temporary; IR owns strings, collections, and values.
- This crate owns portable defaults and validation. Backends receive explicit
  meaning and must not reinterpret wire omissions.
- Error codes and paths are part of the portable behavior.
- Boolean query/filter flattening is canonical IR normalization, not backend
  execution.
- The backend trait is an execution interface, not a second search engine.

If a change appears to require native-engine types here, the boundary is
probably in the wrong place.
