# Search Kernel Web

The default runtime connects to a real multi-index daemon through the public
TypeScript client and its existing gRPC-Web transport. Set
`VITE_SEARCHKERNEL_ENDPOINT` to the daemon's `--web-listen` endpoint (default
`http://localhost:8081`). When the dashboard and daemon are on different
origins, start the daemon with an exact `--web-origin <origin>` (or
`SEARCHKERNEL_GRPC_WEB_ORIGIN`) for the dashboard origin. `--web-listen`
alone does not authorize arbitrary websites to call the mutation-capable API.
`VITE_SEARCHKERNEL_MODE=fixture` explicitly selects an offline demo. Logical
targets come from ListIndexes, and only the selected
workspace is opened. Schema metadata and generation come from OpenIndex.

Search Lab offers Server default for ranking, summary and limit. Those controls
are omitted from canonical requests. Explicit controls, including unknown names,
reach canonical validation. Browsing uses match-all for an empty query and real
text search otherwise. Documents are read through the same generation-bound
handle. Stale handles are reported without automatic reconnection.

Metrics, query inspection and linguistic analysis remain unavailable. No fixture values fill gaps in daemon output. Large
integers and byte/tensor payloads use tagged JSON display values, preserving
precision and payloads.

Verification uses registered container lanes, locally or on an exact pushed head:

```sh
./dev test --remote frontend
./dev test --remote frontend-real-engine
./dev test --remote typescript
./dev test --remote searchkerneld-multi-index
./dev verify --remote frontend.f8
```

The browser integration lane starts a real daemon, creates two targets and a
corpus, restarts it, drives the index picker/Search Lab/document drawer in
Chromium, and checks stale-generation and sibling isolation through the browser
adapter. The immutable F8 harness is invoked separately; its unimplemented
verifier and unstaged dependencies cannot be treated as passing acceptance.

Serving the built SPA requires an index.html fallback for workspace URLs.
Portable network calls remain in the adapter; React does not construct protocol
requests or implement query/schema/ranking semantics.

## Schema and ranking administration (W3)

The schema view displays the complete canonical definition returned by index
open, alongside declared field capabilities. The ranking view inspects complete
profiles and edits first-phase expressions without rebuilding or dropping other
profile properties. Index creation offers primitive fields and explicit stored,
searchable and attribute flags.

Mutation availability comes from `ListIndexesResult.configuration`; an older
server that omits that metadata disables the controls. Validation sends
`PlanIndexChangeRequest` through the generated client and canonical dispatcher.
The backend resolves the definition with shared IR, validates its supported
slice and compiles the exact Vespa matcher synchronously before returning a
plan. Changing the draft invalidates the displayed plan. Apply requires explicit
confirmation and sends only the single-use plan identifier.

Existing-index apply supports rank-profile changes. Field/schema identity,
summary, constant and model changes produce a plan with unsupported reasons;
there is no reindex implementation. The backend owns the target transition,
closes the previous engine, publishes configuration with a durable rollback
journal, reopens retained document data and returns a fresh generation. Reopen
restores an interrupted publication before opening Vespa. Old handles fail
rather than silently switching generation. The dashboard replaces metadata and
clears results from the old incarnation after an explicit apply or reopen.

Plans expire on process restart or bounded-cache eviction and must be validated
again. F7 and F9 acceptance verifiers remain immutable `NOT_IMPLEMENTED`
placeholders; ordinary test results do not establish those milestones.

## Index lifecycle and diagnostics (W4)

Index administration uses canonical CreateIndex, OpenIndex, DeleteIndex,
ListIndexes and Describe through SearchKernelClient. Create loads the index;
opening a loaded index returns its existing generation. Generations identify
loaded process incarnations, and change after restart/reopen or delete/recreate.
Workspace URLs retain `generation`; an explicit open establishes a new target.
A stale URL never silently upgrades a mutation to a replacement incarnation.

`POST /admin/status` reports typed registry/storage diagnostics without loading
indexes. W3 canonical PlanIndexChange validates and compiles creation definitions before
CreateIndex. The shared W3 editor supports field controls and an advanced
standard FlatBuffers JSON mode. The
read-only diagnostics surface does not expose environment variables, paths, logs,
credentials, or host files. Counts, resource metrics, runtime version and lifecycle
history remain unavailable. Native status means the runtime is linked; it is not
an invented engine health probe. Transitioning targets have unknown persistence
until a completed lifecycle snapshot can establish it.

Delete confirmation captures the exact canonical target and requires its logical
name. The bound client includes the captured generation in DeleteIndex. Failed or
lost lifecycle responses are never optimistic successes; refresh authoritative
inventory before retrying. Per-index operation state and generation-scoped caches
preserve sibling independence. Workbench state remounts across incarnations.


W3 configuration operations consume the same canonical selected target. Plan/apply
uses a dedicated generation-bound SDK client so an apply response cannot upgrade
a cached historical handle. Successful apply/reopen establishes the returned
incarnation in the URL and clears only the prior incarnation's dependent views.
Opening a client handle remains distinct from unloading the shared daemon engine.
