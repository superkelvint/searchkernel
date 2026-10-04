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


Index administration uses canonical CreateIndex, OpenIndex, DeleteIndex,
ListIndexes and Describe through SearchKernelClient. Create loads the index;
opening a loaded index returns its existing generation. Generations identify
loaded process incarnations, and change after restart/reopen or delete/recreate.
Workspace URLs retain `generation`; an explicit open establishes a new target.
A stale URL never silently upgrades a mutation to a replacement incarnation.

`POST /admin/status` reports typed registry/storage diagnostics without loading
indexes. `POST /admin/validate-definition` runs canonical IndexDefinition JSON
through the BFBS bridge and shared IR resolver without creating storage. These
read-only admin surfaces do not expose environment variables, paths, logs,
credentials, or host files. Counts, resource metrics, runtime version and lifecycle
history remain unavailable. Native status means the runtime is linked; it is not
an invented engine health probe. Transitioning targets have unknown persistence
until a completed lifecycle snapshot can establish it.

Delete confirmation captures the exact canonical target and requires its logical
name. The bound client includes the captured generation in DeleteIndex. Failed or
lost lifecycle responses are never optimistic successes; refresh authoritative
inventory before retrying. Per-index operation state and generation-scoped caches
preserve sibling independence. Workbench state remounts across incarnations.
