# M11 source trace

The native lifetime and concurrency checks trace to the `EmbeddedEngine`
resource members, `DocumentDB` startup/close path, persistence handler setup,
and the feed/reopen behavior in `core/native/vespa/searchkernel_vespa.cpp`.

The public ABI source is `core/native/vespa/searchkernel_vespa.h`. The Rust raw
declarations are generated from that header and the hardening controls are
feature-gated in `searchkernel-vespa-bindings`. M8–M10 result, grouping, and
ranking paths are reached through the same native generic/local calls. The
adapter trace is `core/crates/searchkernel-ir-adapter/src/lib.rs`; production
C5 calls enter through `core/crates/searchkernel-ffi/src/{abi,dispatch,buffer}.rs`.

Behavioral expectations for mutation, text matching, query lowering, schema,
and persistence are inherited from the frozen M0–M10 contracts and the pinned
Vespa checkout. M11 does not add an alternative search oracle.
