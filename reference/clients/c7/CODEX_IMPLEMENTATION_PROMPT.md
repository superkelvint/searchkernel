Implement C7 against the frozen verifier in `reference/clients/c7/`.

Do not modify verifier files or expected outputs.

Required architecture:

```text
language facade
  -> generated client core
  -> NativeTransport
  -> C5 byte-oriented ABI
  -> canonical FlatBuffers dispatcher
```

Implement representative Python and Go NativeTransport support. Keep transports
strictly limited to library loading, ABI version checking, handle lifecycle,
`call(request bytes) -> response bytes`, native response-buffer ownership/free,
and packaging.

Do not put Search Kernel defaults, filters, ranking, facets, portable validation,
operation definitions, portable errors, JSON semantics, or request parsing into
NativeTransport.

Provide the tiny verifier adapters required by the contract:

```text
clients/python/tests/c7_adapter.py
clients/go/c7adapter/
```

Those adapters must delegate to production code; they may not implement FFI.

Make `reference/clients/c7/verify-c7.sh` pass without weakening C5/C6 or adding
per-operation C ABI functions.
