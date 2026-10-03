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

Metrics, health, diagnostics, query inspection and linguistic analysis are
unavailable in this slice. No fixture values fill gaps in daemon output. Large
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
