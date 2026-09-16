# M11 oracle boundary

M11 has no alternate semantic search oracle. The only test-side oracle is the
logical document-state model in the Rust stress driver. Search results are
produced by the real embedded Vespa engine and checked using M0–M10 behavior.
Direct-engine, IR-adapter, and production-C5 lanes share the same deterministic
workload shape; adapter and C5 are delivery-path checks, not independent
semantic implementations.
