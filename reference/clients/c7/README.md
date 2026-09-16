# C7 Python NativeTransport verifier

This directory freezes the verifier for **C7 — Native embedded client
transport**, using Python as the single representative foreign-language proof.

Run from the repository root:

```bash
reference/clients/c7/verify-c7.sh
```

## Before freezing the verifier commit

Reconcile:

```text
reference/clients/c7/shim/c5_abi_adapter.h
```

with the already-frozen C5 public header. C7 intentionally centralizes C5 ABI
spelling in that one verifier-only file.

## Required implementation adapter

The verifier expects:

```text
clients/python/tests/c7_adapter.py
```

Use `python/C7_ADAPTER_CONTRACT.py` as the required shape.

The adapter exists only to decouple this frozen verifier from cosmetic package
layout/import changes. It must delegate to the production `NativeTransport`
and generated client core. It may not implement FFI itself.

## Real C5 test library

The verifier runs C5 and C6 prerequisite verifiers, then requires the actual C5
test library path through:

```bash
SEARCHKERNEL_C5_TEST_LIBRARY=/absolute/path/to/libsearchkernel_ffi.so \
  reference/clients/c7/verify-c7.sh
```

This is explicit because a child shell verifier cannot export environment
variables back into its parent shell.

If the repository has a stable deterministic path for the C5 test library, you
may freeze that path in `verify-c7.sh` before committing the C7 specification.
Do not make C7 guess among arbitrary Cargo artifacts.

## What is verified

The plumbing shim verifies:

- ABI check before native use;
- byte-for-byte request/response fidelity;
- embedded NUL and 1 MiB payloads;
- explicit and context-manager lifecycle;
- idempotent close and local use-after-close rejection;
- independent handles;
- native failure separation;
- native response copy/free ownership;
- 5,000-call leak/double-free stress;
- exact native call/open/close/allocation counts.

The real-C5 smoke verifies representative generated-client operations through
the real canonical dispatcher using C3 recording/scripted backends.
