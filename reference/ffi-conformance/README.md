# C5 FFI conformance verifier

This directory is the normative verifier for **C5 — Portable native ABI**.

Run from the repository root:

```bash
./reference/ffi-conformance/verify.sh
```

It verifies five independent boundaries:

1. **ABI shape** — exact header, four production symbols, stable numeric constants, primitive output parameters, and tokenized buffer ownership.
2. **Dependency isolation** — no Vespa or testkit dependency in the normal FFI crate.
3. **Transport safety** — verified FlatBuffers only; malformed/truncated bytes never reach the backend.
4. **Semantic routing** — requests cross the real `*_to_ir` path and arrive at C3 backends as exact IR.
5. **Ownership/lifetime** — handle invalidation, no reuse, buffer free/zeroing, panic containment, concurrent close.

The verifier intentionally does not test search behavior. `searchkernel-testkit` supplies scripted or recording backends only.

## Required implementation files

C5 must provide:

```text
core/crates/searchkernel-ffi/
    Cargo.toml
    include/searchkernel_ffi.h
    src/...

api/searchkernel_ffi_request.fbs
api/searchkernel_ffi_response.fbs
```

The header and both transport schemas must match the checked-in reference
copies in this directory exactly.

The crate must build a `cdylib` named `searchkernel_ffi` and expose the Rust-only `ffi-test-support` feature described in `TEST_SUPPORT_CONTRACT.md`.
