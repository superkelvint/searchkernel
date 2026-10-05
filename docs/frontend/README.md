# Frontend F-series verification

The F-series covers the future Search Kernel dashboard only. This repository
contains its acceptance boundary, not the dashboard implementation.

The required product layering is:

```text
searchkernel-web
  → clients/typescript
  → searchkerneld
  → canonical dispatcher
  → searchkernel-ir
  → searchkernel-ir-adapter
  → searchkernel-vespa-engine
```

Portable operations use the generated TypeScript client. A narrowly scoped
admin client may call process diagnostics. The dashboard must not implement
Search Kernel semantics, import internal core crates or raw FlatBuffers, infer
capabilities from versions, or emulate unsupported backend behavior.

## Milestones

| Milestone | Definition |
|---|---|
| F0 | Frontend foundation |
| F1 | Dashboard development backend |
| F2 | Application shell/index workspace |
| F3 | Documents |
| F4 | Search Lab |
| F5 | Search debugging |
| F6 | Read-only configuration |
| F7 | Configuration mutation/change planning |
| F8 | Real-engine integration |
| F9 | Ranking and summaries |
| F10 | Operations/diagnostics |
| F11 | Higher-level search behavior |
| F12 | Product hardening |

The dependency chain is declared in each immutable manifest. Missing future
paths are `NOT_APPLICABLE`; an unimplemented verifier is `NOT_IMPLEMENTED` and
always blocks acceptance.

## Authority

The immutable verifier, architecture gates, clean candidate verification, and
independent reviewer results decide completion. Run:

```bash
tools/frontend-harness/frontend-accept F4
```

Only that command may record `passed`.

## W-series integration launch

[W6 — Integration & Application Launch](W6.md) records the selected-index
consumer workspace, canonical InstantSearch authority, live public-endpoint
preview, and the remaining endpoint-management/durable-binding dependencies.
W6 does not alter F-series frozen acceptance or own compatibility semantics.
