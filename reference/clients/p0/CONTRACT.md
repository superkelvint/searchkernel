# P0 — Usable Python vertical slice

P0 proves the handwritten Python product façade against the production C5
library and the real embedded Search Kernel engine. The acceptance program is
`acceptance.py` and must remain a direct Python-to-disk integration path.

The required architecture is:

```text
Python façade
  -> generated Python Portable API client
  -> NativeTransport
  -> production C5 ABI
  -> canonical dispatcher
  -> real Search Kernel backend
```

The verifier must build `searchkernel-ffi` with the `production` feature. C7
recording/scripted fixtures, Rust callers, mocks, and alternate search or
storage implementations are not valid substitutes.

Native library resolution is tested in this order:

1. `SEARCHKERNEL_NATIVE_LIBRARY` when explicitly set;
2. the bundled Linux x86-64 package resource;
3. a clear failure when neither is available.

The Python façade may convert Python values, construct generated request model
objects, and wrap generated results. Protocol encoding/decoding and Portable
API semantics remain owned by the generated layer.
