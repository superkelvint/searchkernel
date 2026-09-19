# C6 conformance verifier

This directory is the normative acceptance harness for **C6 — BFBS-driven generation and retained JSON codec**.

Run from the repository root:

```bash
./reference/json-conformance/verify.sh
```

The complete verifier proves:

1. canonical `.fbs` schemas reproducibly compile to committed BFBS with `flatc 25.12.19`;
2. `searchkernel-bindgen` consumes BFBS and deterministically generates the C6 artifact tree;
3. BFBS is genuinely the generator input, proven with isolated BFBS-only mutations after the temporary `.fbs` sources are deleted; unknown fields and variants fail closed until their JSON dispositions are defined;
4. generated BFBS-derived Python domain models and schema metadata cover the canonical request/response surface;
5. matching SKRS responses decode through the retained JSON codec and portable SKRS errors remain portable errors;
6. public JSON is Search Kernel canonical JSON, not official FlatBuffers JSON;
7. JSON preserves presence, uses symbolic one-key unions, decimal-string int64/uint64, base64 bytes, and strict decoding;
8. the generated schema inventory, operation map, and frozen binary fixture manifest are synchronized;
9. generator completeness is fail-closed: unknown BFBS fields and enum/union variants, an unpaired operation, and malformed BFBS are rejected with diagnostics identifying the missing disposition or invariant;
10. JSON→FlatBuffers and native FlatBuffers use the exact same dispatcher and produce identical Search Kernel IR;
11. the full `reference/api-conformance` semantic suite remains green.

The existing BFBS reflection bridge is useful groundwork, but official FlatBuffers JSON by itself does **not** satisfy C6.

See `CONTRACT.md` for the frozen CLI and public contracts.
