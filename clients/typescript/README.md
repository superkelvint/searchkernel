# Search Kernel TypeScript client

`@searchkernel/client` is the browser-capable TypeScript facade over Search
Kernel's canonical FlatBuffers contract. It expects a gRPC-Web-compatible
`SearchKernel.Execute(FlatBufferFrame)` endpoint; configuring the daemon for
gRPC-Web is outside this package.

```ts
import { Index, Tensor, agg, float64, q } from "@searchkernel/client";

const index = Index.connect("https://search.example.com", "products");

await index.upsert({
  _id: "shoe:1",
  title: "Trail shoe",
  price: float64(129),
  embedding: Tensor.dense("tensor<float>(x[2])", [0.2, 0.8]),
});

const results = await index
  .query(q.allOf(
    q.contains("title", "shoe"),
    q.nearestNeighbor("embedding", "query_embedding"),
  ))
  .filter("price <= :max", { max: 150 })
  .input("query_embedding", Tensor.dense("tensor<float>(x[2])", [0.1, 0.9]))
  .facet("brand", { limit: 10 })
  .aggregate(agg.avg("price").as("avg_price"))
  .limit(10)
  .execute();

console.log(results.hits, results.facets, results.aggregations);
```

## Numeric values

JavaScript has only one `number` type, so `129` and `129.0` are indistinguishable.
Ordinary integral `number` values therefore encode as signed `int64`; fractional
values encode as `float64`. When a schema expects Float64 but the value happens
to be integral, use the explicit helper:

```ts
import { float64 } from "@searchkernel/client";

await index.upsert({
  _id: "shoe:1",
  price: float64(129),
});
```

## 64-bit integers

Integral JavaScript `number` values are accepted only while they are safe
integers. `bigint` encodes signed `int64`. For the full signed or unsigned range,
including lossless decimal-string input, use the explicit helpers:

```ts
import { int64, uint64 } from "@searchkernel/client";

await index.upsert({
  _id: "counter:1",
  signed: int64("9223372036854775807"),
  unsigned: uint64("18446744073709551615"),
});
```

An ordinary JavaScript string remains a Search Kernel string; it is never
silently reinterpreted as an integer. Decoded `int64`/`uint64` values are
returned as `bigint`.

## Target serverless deployment architecture

Search Kernel should eventually support a deployment model where a TypeScript
application can use serious local search without requiring the application
operator to provision and continuously run a traditional search server.

The important boundary is that `searchkerneld` does **not** run inside an
ordinary stateless serverless function or edge isolate. The target is a
serverless container or microVM with a real Linux process model and storage
whose durability semantics are sufficient for the Vespa index.

Conceptually:

```text
Next.js / TypeScript application
        ↓
@searchkernel/client
        ↓
serverless control/router layer
        ↓
named SearchKernel sandbox/container
        ↓
searchkerneld
        ↓
persistent index storage
```

The portable TypeScript client remains unchanged by this deployment mode. It
still talks to the daemon through a supported Search Kernel transport and owns
no separate search semantics.

### Vercel target

As of September 2026, Vercel Sandbox is the most promising concrete target.
Persistent Sandboxes can preserve filesystem state between sessions, and Vercel
Sandbox Drives provide separately managed persistent storage that can be mounted
into a Sandbox as a normal directory. A Drive can move between sandbox
instances, while read-write ownership is restricted to one Sandbox at a time.

That maps naturally onto Search Kernel's single engine-owner model:

```text
Vercel / Next.js request
        ↓
resolve SearchKernel index identity
        ↓
resume or create named Sandbox
        ↓
mount index Drive
        ↓
start/resume searchkerneld
        ↓
serve requests
        ↓
idle / stop compute
        ↓
persistent index remains
```

A future Vercel integration should aim for an application-facing experience
similar to:

```ts
const index = SearchKernel.open("products");
await index.search(...);
```

without requiring the application developer to operate an Elasticsearch,
OpenSearch, Solr, or permanently running SearchKernel host.

This remains a feasibility target, not a supported deployment today. Before
Vercel Drive can be treated as a supported Search Kernel persistence backend,
the native engine must be tested adversarially against its actual filesystem
semantics and performance. At minimum verify mmap behavior where required, file
locking, atomic rename, fsync/durability, crash recovery, transaction-log
reopen, random I/O, hard termination, remount onto a different Sandbox, and
exclusive writer behavior.

Relevant platform documentation:

- Vercel Sandbox persistence:
  <https://vercel.com/changelog/sandbox-persistence-is-now-ga>
- Vercel Sandbox Drives:
  <https://vercel.com/changelog/drives-for-vercel-sandbox-are-now-in-public-beta>

### Cloudflare target

Cloudflare Workers themselves are not a target host for the native engine.
SearchKernel depends on a real native multithreaded process and local filesystem
behavior that should not be forced into an edge-isolate/Wasm architecture.

Cloudflare Containers are more interesting because they provide a Linux
container runtime that can host `searchkerneld`. Their local disk is currently
ephemeral, however, so they do not yet provide the same straightforward
persistent-index model.

R2 can be mounted through FUSE, but object storage is not equivalent to a POSIX
local SSD and must not be treated as a supported Vespa index filesystem without
specific verification.

A possible future Cloudflare architecture is therefore a reconstructible local
index:

```text
Cloudflare Worker
        ↓
Durable Object / index identity
        ↓
SearchKernel Container
        ↓
ephemeral local SSD index
        ↑
checkpoint + durable mutation state
        ↓
R2 / durable storage
```

In that design the local Vespa index is an execution replica/cache that can be
rebuilt from a durable checkpoint plus mutations. This is a materially different
persistence architecture and must remain separate from the ordinary Search
Kernel local-persistence contract unless and until it is explicitly designed and
verified.

Relevant platform documentation:

- Cloudflare Container disk lifecycle:
  <https://developers.cloudflare.com/containers/faq/>
- R2 FUSE mounts and their storage-semantics warning:
  <https://developers.cloudflare.com/containers/examples/r2-fuse-mount/>

### Netlify and ordinary serverless functions

Ordinary Netlify Functions currently execute in an ephemeral runtime and have
bounded execution lifetimes. They are therefore not an appropriate host for a
persistent `searchkerneld` process.

The same rule applies generally to conventional request-scoped serverless
functions: they may be excellent **clients** of SearchKernel, but the engine
needs a durable container/microVM-style execution surface or an external managed
daemon.

If Netlify or another platform introduces durable container execution plus
storage suitable for native database/search-engine filesystem semantics, it can
be evaluated using the same persistence and lifecycle acceptance criteria rather
than requiring a new Search Kernel semantic path.

Relevant platform documentation:

- Netlify Functions runtime model:
  <https://docs.netlify.com/build/functions/overview/>
- Netlify function execution limits:
  <https://docs.netlify.com/build/functions/configuration/>

### Product invariant

"Serverless SearchKernel" should mean:

```text
disposable / scale-to-zero compute
+ durable index ownership
+ one canonical searchkerneld execution path
```

It must **not** mean porting SearchKernel semantics into JavaScript, embedding
Vespa separately in every function invocation, silently weakening persistence,
or maintaining a platform-specific query/indexing implementation.

## Development

From `clients/typescript`:

```text
npm run generate
npm run check:generated
npm run typecheck
npm run build
npm test
npm run test:package
```

Generation is reproducible from the canonical `.fbs` and `.proto` files. The
build bootstraps generated output when it is absent. `check:generated` creates
an independent fresh tree and compares it with the output consumed by the
package, rather than relying on Git to observe ignored files. The SDK does not
parse filter expressions or reimplement Search Kernel defaults, validation,
ranking, grouping, or query semantics.

## Managed local Node.js usage

Browser and Node clients select a logical index through canonical request envelopes. Use `Index.create(endpoint, target, definition)`, `Index.open(endpoint, target)`, or `Index.connect(endpoint, target)` with an externally managed daemon. Node also retains path-based `create(path, options)` and `open(path)` helpers. Managed-local keeps daemon authority inside a dedicated `.searchkernel-managed` data root beside the caller path, derives a stable canonical target from the full path, and shares one daemon across sibling indexes. Existing legacy path-owned indexes are validated and migrated into that managed root on first open; arbitrary path names remain valid. Installed npm packages do not bundle the daemon, so managed-local calls require `SEARCHKERNEL_LOCAL_HOST` to name an existing binary. A source checkout may use the development fallback `target/host/debug/searchkerneld`.

The complete managed-local showcase lives at `examples/try_searchkernel_client_api_e2e.ts`. After building `searchkerneld`, run it with:

```bash
npm run example:e2e
```
