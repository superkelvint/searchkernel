# C6 conformance verifier

This directory is the normative acceptance harness for **C6 — BFBS-driven generation and portable client core**.

Run from the repository root:

```bash
./reference/json-conformance/verify.sh
```

The complete verifier proves:

1. canonical `.fbs` schemas reproducibly compile to committed BFBS with `flatc 25.12.19`;
2. `searchkernel-bindgen` consumes BFBS and deterministically generates the C6 artifact tree;
3. BFBS is genuinely the generator input, proven with an isolated synthetic-schema mutation after the temporary `.fbs` sources are deleted;
4. generated synchronous and asynchronous client operations use one transport call boundary and emit the correct SKRQ variants;
5. matching SKRS responses decode through both generated clients and portable SKRS errors become portable client errors;
6. public JSON is Search Kernel canonical JSON, not official FlatBuffers JSON;
7. JSON preserves presence, uses symbolic one-key unions, decimal-string int64/uint64, base64 bytes, and strict decoding;
8. generated operation metadata, JSON Schema/OpenAPI-compatible metadata, docs, and corpus fixtures are synchronized;
9. generator completeness is fail-closed: a new valid BFBS operation/field is generated, while an unpaired operation and malformed BFBS are rejected;
10. JSON→FlatBuffers and native FlatBuffers use the exact same dispatcher and produce identical Search Kernel IR;
11. the full `reference/api-conformance` semantic suite remains green.

The existing BFBS reflection bridge is useful groundwork, but official FlatBuffers JSON by itself does **not** satisfy C6.

See `CONTRACT.md` for the frozen CLI and public contracts.
