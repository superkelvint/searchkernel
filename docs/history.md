# History and compatibility context

This document explains why older milestone names, verifier artifacts, and
transport references remain in the repository. It is historical context, not a
statement of current implementation status.

## The roadmap's former endpoint

The original low-level roadmap treated M11, “low-level wrapper hardening,” as
the end of the embedded Vespa track. M12 and M13 were added later for dense
query-tensor execution and dense tensor document CRUD. The old M11 endpoint is
therefore a historical checkpoint, not the current project endpoint.

## The transport migration

The former non-Rust design used a portable client C ABI and a per-language
`NativeTransport`. That design was superseded by:

```text
language facade
    -> generated FlatBuffers models
    -> generic gRPC frame
    -> searchkerneld
    -> shared dispatcher
```

The Rust-to-Vespa native boundary is not retired. Only the portable
foreign-language transport path was replaced. References to the old ABI in
contracts, implementation plans, or source traces explain migration history;
they do not authorize extending that path.

## Why old artifacts remain

- Frozen contracts and verifiers under `reference/` preserve acceptance
  history and cumulative behavior.
- `docs/implementation-plan.md` records the decisions and sequencing used to
  build the current system; it is not a live status board.
- Git history preserves superseded designs and the reasoning behind their
  removal.

Do not edit a frozen acceptance artifact merely to make the current
implementation appear further along. Update the implementation and record the
verification result in status/reporting documentation instead.

For current facts, use [Current status](status.md). For invariant boundaries,
use [Architecture](architecture.md). For intended future work, use the
[Roadmap](roadmap.md).
