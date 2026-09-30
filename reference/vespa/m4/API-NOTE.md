# API freeze note

`core/crates/searchkernel-vespa-engine/examples/api-contract-m4.rs` deliberately freezes only the
**additive M4 text surface**. It does not restate M1 engine construction or M2
delete syntax.

Before freezing M4, compile this fixture against the current workspace and make
only verifier-authoring adjustments needed to align imports/error alias with
the previously committed earlier API. Once the M4 verifier specification is
committed, the M4 implementation agent may not change it.

The result currently includes `score: f64` because the M4 contract asks to prove
BM25. If the project wants to defer public raw score exposure to M8, change the
fixture and Rust driver **now, before freezing M4**, and verify BM25 via oracle
ranking order plus a native-only rank-profile assertion instead.
