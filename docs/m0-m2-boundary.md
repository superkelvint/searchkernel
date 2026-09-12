# M0–M2 boundary

M0–M2 intentionally use the compact private C ABI in `native/vespa/`.
The Rust call path is:

```text
searchkernel public API
    -> searchkernel-vespa safe adapter
    -> searchkernel-sys raw ABI
    -> native/vespa Search Kernel adapter
    -> Vespa PersistenceEngine / DocumentDB / Matcher
```

The protobuf in `api/searchkernel.proto` is private, language-neutral plumbing
for later client and runtime layers. It is not used by the M0–M2 execution
path. Structured query expressions are represented by `QueryExpr` when those
layers begin compiling public query builders; the existing `query` string is
reserved for text-query syntax.

`searchkernel-core` is the backend-independent semantic layer for the C2
milestone. It remains independent of the Vespa-specific crates.
