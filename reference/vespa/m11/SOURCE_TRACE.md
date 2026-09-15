# M11 source trace

The native lifetime and concurrency checks trace to the `EmbeddedEngine`
resource members, `DocumentDB` startup/close path, persistence handler setup,
and the feed/reopen behavior in `native/vespa/searchkernel_vespa.cpp`.

The public ABI source is `native/vespa/searchkernel_vespa.h`. The Rust raw
declarations are generated from that header and the hardening controls are
feature-gated in `searchkernel-vespa-sys`.

Behavioral expectations for mutation, text matching, query lowering, schema,
and persistence are inherited from the frozen M0–M7 contracts and the pinned
Vespa checkout. M11 does not add an alternative search oracle.
