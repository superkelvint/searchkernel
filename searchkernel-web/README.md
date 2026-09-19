# Search Kernel Admin Dashboard — static first pass

A static React + TypeScript + Vite prototype for the Search Kernel Admin Dashboard.

The development entry point uses representative local data through fixture implementations of the generated-client-shaped boundaries. The browser owns query state and display only; request, filter, schema, ranking, and change-plan semantics remain backend concerns.

## Run locally

```bash
npm install
npm run dev
```

Build the production static bundle with:

```bash
npm run build
```

The resulting `dist/` directory is suitable to be served as static assets by `searchkerneld`.

### Production routing requirement

This preview uses `BrowserRouter`. `searchkerneld` must serve concrete files from `dist/` first, keep `/api/*` on the API transport, and fall back to `dist/index.html` for every other unmatched request. That fallback is required for direct loads and reloads of routes such as `/indexes/products/search/lab?q=wireless+headphones`.

## Included prototype surfaces

- Parameterized React Router route tree with explicit canonical-child redirects and 404 handling
- Global and index overview
- Document browser with URL-backed client search and a keyboard-accessible document drawer
- Search Lab with URL-backed query/ranking/limit state, committed-request results, opt-in debounced live preview, Cmd/Ctrl+Enter, and read-only request JSON/curl inspection
- Client-backed Analyze; Query Inspector is deliberately unavailable until the server exposes an inspection response
- Schema, Rank Profiles and Summaries
- Index and global operations, diagnostics and settings
- Command palette (`Cmd/Ctrl+K`)

The dashboard has two explicit client seams. `src/api/searchkernel-client.ts` defines the generated-client-shaped `SearchKernelClient` contract for portable operations; `src/admin-client/admin-client.ts` defines the separately scoped `AdminClient` contract for operational endpoints. The fixture providers implement those contracts and are composed only in `src/main.tsx`, so features never import fixture storage. Replacing the providers with generated and daemon-backed clients does not change feature code. The UI does not infer product support from a server version.

Catalyst components from the supplied Tailwind Plus kit are included under `src-components/`; dialog primitives are used directly and the rest of the UI follows its Tailwind visual patterns.
