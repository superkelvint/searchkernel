# `searchkernel-ffi`

This crate exposes the small C ABI used by non-Rust callers. It frames
FlatBuffers requests, verifies them, routes them through `searchkernel-core`,
and returns owned response buffers.

## Start reading here

1. Read `include/searchkernel_ffi.h` for the C-facing contract.
2. Read `src/lib.rs` for the exported primitive ABI declarations and module
   layout.
3. Read `src/abi.rs` for panic containment and ABI entry points.
4. Read `src/transport.rs` for envelope verification and framing.
5. Read `src/dispatch.rs` for operation routing.
6. Read `src/buffer.rs` and `src/handle.rs` for ownership and lifetime rules.

## Key concepts

- Raw pointers are valid only for the duration and purpose stated by the C
  ABI. Validate them before entering ordinary Rust code.
- Rust panics and C++ exceptions must not cross the ABI.
- Response bytes are allocated by Search Kernel and must be released with the
  matching pointer/token free function; no response descriptor struct crosses
  this ABI.
- Envelope verification happens before a payload reaches conversion or a
  backend.

This boundary is intentionally small. Keep ordinary semantics in
`searchkernel-core` and avoid adding product behavior here.
