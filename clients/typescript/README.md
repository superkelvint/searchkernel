# Search Kernel TypeScript client

`@searchkernel/client` is the browser-capable TypeScript facade over Search
Kernel's canonical V2 FlatBuffers contract. It expects a gRPC-Web-compatible
`SearchKernel.Execute(FlatBufferFrame)` endpoint; configuring the daemon for
gRPC-Web is outside this package.

```ts
import { Index, Tensor, agg, float64, q } from "@searchkernel/client";

const index = Index.connect("https://search.example.com");

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

## Low-level generated API

Capabilities represented by the canonical FlatBuffers contract but not yet
wrapped ergonomically remain available through the supported low-level export:

```ts
import { SearchRequestT, ValueT } from "@searchkernel/client/low-level";
```

This is a generated wire-model escape hatch only; it does not define separate
client-side Search Kernel semantics.

## Development

From `clients/typescript`:

```text
npm run generate
npm run typecheck
npm run build
npm test
npm run test:package
```

Generation is reproducible from the canonical `.fbs` and `.proto` files. The
SDK does not parse filter expressions or reimplement Search Kernel defaults,
validation, ranking, grouping, or query semantics.

## Managed local Node.js usage

Browser code continues to use the main `@searchkernel/client` entrypoint and `Index.connect(...)`.
Node.js can use `@searchkernel/client/node` to mirror the Python client lifecycle: `Index.create(...)` and `Index.open(...)` spawn and own a local `searchkerneld` over a private Unix-domain socket. Set `SEARCHKERNEL_LOCAL_HOST` to an explicit daemon binary when needed; the repository fallback is `target/host/debug/searchkerneld`.

The complete managed-local showcase lives at `examples/try_searchkernel_client_api_e2e.ts`. After building `searchkerneld`, run it with:

```bash
npm run example:e2e
```
