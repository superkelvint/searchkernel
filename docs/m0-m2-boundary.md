# M0–M2 boundary

M0–M2 intentionally use the compact private C ABI in `core/native/vespa/`.
The Rust call path is:

```text
searchkernel public API
    -> searchkernel-vespa-engine safe adapter
    -> searchkernel-vespa-bindings raw ABI
    -> core/native/vespa Search Kernel adapter
    -> Vespa PersistenceEngine / DocumentDB / Matcher
```

The FlatBuffers contract in `api/searchkernel.fbs` is private, language-neutral
plumbing for later client and runtime layers. It is not used by the M0–M2 execution
path. Structured query expressions are represented by `QueryExpr` when those
layers begin compiling public query builders; the existing `query` string is
reserved for text-query syntax.

`searchkernel-ir` performs API → IR conversion and owns the backend-
independent Search Kernel IR. It remains independent of the Vespa-specific
crates.
