# API freeze note

`reference/vespa/api-contract/src/bin/m4.rs` deliberately freezes only the
**additive M4 text surface**. It does not restate M1 engine construction or M2
delete syntax.

Before freezing M4, compile this fixture against the current workspace and make
only harness-authoring adjustments needed to align imports/error alias with the
already-frozen earlier API. Once `M4-IMMUTABLE.sha256` is created, the M4
implementation agent may not change it.

The result currently includes `score: f64` because the M4 contract asks to prove
BM25. If the project wants to defer public raw score exposure to M8, change the
fixture and Rust driver **now, before freezing M4**, and verify BM25 via oracle
ranking order plus a native-only rank-profile assertion instead.
