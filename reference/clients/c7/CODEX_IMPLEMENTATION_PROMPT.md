Implement C7 against the frozen verifier in `reference/clients/c7/`.

Do not modify verifier files or expected outputs.

Required architecture:

```text
Python facade
  -> generated Python client core
  -> NativeTransport
  -> C5 byte-oriented ABI
  -> canonical FlatBuffers dispatcher
```

Implement Python `NativeTransport` only. Use the normal Python mechanism chosen
by the repository for loading/calling the existing C5 shared library (for
example `ctypes` if that is the project decision). Keep the production
transport strictly limited to:

- library loading;
- ABI version checking;
- handle lifecycle;
- `call(request bytes) -> response bytes`;
- native response-buffer copy/free ownership;
- deterministic close/context-manager support;
- packaging/library discovery.

Do not put Search Kernel defaults, filters, ranking, facets, portable
validation, operation definitions, portable errors, JSON semantics,
FlatBuffers parsing, or IR conversion into `NativeTransport`.

Provide:

```text
clients/python/tests/c7_adapter.py
```

matching `reference/clients/c7/python/C7_ADAPTER_CONTRACT.py`.

The adapter must delegate to production code; it may not implement FFI itself.
Its `real_c5_smoke()` must exercise at least one mutation-like operation, one
search operation, and one describe/inspect operation through the generated
Python client core against the real C5 test dispatcher/C3 fixtures. It must
also prove that a scripted Portable API error is mapped by generated client
code rather than NativeTransport.

Make `reference/clients/c7/verify-c7.sh` pass without weakening C5/C6, adding a
second semantic path, or adding per-operation C ABI functions.
