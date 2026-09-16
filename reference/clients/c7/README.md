# C7 verifier package

This directory freezes the verifier for **C7 — Native embedded client
transport**.

Run from the repository root:

```bash
reference/clients/c7/verify-c7.sh
```

Before freezing the verifier commit, reconcile `shim/c5_abi_adapter.h` with the
already-frozen C5 public header. C7 intentionally centralizes that dependency in
one file.

The verifier expects implementation adapters at:

```text
clients/python/tests/c7_adapter.py
clients/go/c7adapter/
```

Those adapters are intentionally tiny and exist only to decouple the frozen
verifier from cosmetic package-layout changes produced by C6. They must merely
construct the production `NativeTransport` and expose `open/call/close` to the
probe; they may not implement FFI themselves.

The real-C5 smoke step is repository-specific. `verify-c7.sh` looks for:

```text
reference/ffi-conformance/verify-c5.sh
reference/clients/c6/verify-c6.sh
```

and then invokes the two C7 adapters in `real-c5-smoke` mode. If your C5/C6
verifier entry points use different frozen paths, change the two variables at
the top of `verify-c7.sh` before committing the C7 specification.
