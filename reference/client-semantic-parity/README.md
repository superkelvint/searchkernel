# Cross-language canonical IR parity

`scenarios.json` is the immutable intent corpus for SK-23-016. Each scenario is
constructed independently through the public Rust, Python, and TypeScript
facades; language syntax is allowed to differ. The gate compares the resulting
`searchkernel-ir::snapshot::search_request` JSON, so semantic resolution remains
owned exclusively by `searchkernel-ir`.

Run `python3 tools/check-client-semantic-parity.py`. Capability gaps are explicit
corpus data rather than silently omitted equivalence claims. Response/result
parity remains owned by #141; real-engine cross-entry parity remains owned by
#314.
