# Frontend and F-series guidance

Frontend dashboard work is documented under `docs/frontend/` and is
implemented only in `searchkernel-web/`, `clients/typescript/`, and the
appropriate daemon integration. The immutable acceptance machinery lives in
`tools/frontend-harness/`.

F-series rules:

- TypeScript must not implement Search Kernel semantics.
- Portable operations use the generated TypeScript client.
- A narrowly scoped admin client may call process diagnostics; it must not
  become a second portable API.
- Dashboard code must not import internal `core/` crates or raw FlatBuffers.
- Normal implementation work must not modify milestone verifiers, fixtures,
  goldens, expected IR, manifests, reviewer contracts, or result schemas.
- Unsupported capabilities are gated, never emulated.
- Feature availability is capability-driven, not version-driven.
- `frontend-accept <F#>` is the authority for F-series completion.

The dashboard is a presentation and administration layer over Search Kernel
APIs. Keep request, filter, schema, ranking, and change-plan semantics in the
backend and preserve the frontend layering through the generated client and
daemon integration.
