# Search Kernel Ruby client

This package is the Ruby first-party client for Search Kernel.

Ruby does not have an upstream `flatc` code generator, so this client uses the
SOP-sanctioned standard FlatBuffers JSON transport instead of defining a Ruby
schema or handwritten FlatBuffer binary codec:

```text
Ruby client
  -> standard FlatBuffers JSON RequestEnvelope
  -> POST /v1/execute
  -> searchkerneld BFBS-backed parser
  -> canonical FlatBuffer bytes
  -> shared dispatcher / searchkernel-ir
```

The client exposes the low-level operation envelope, portable/protocol error
separation, and HTTP transport. `SearchKernel::Value.encode` builds canonical
Value union JSON from Ruby scalars, lists and string-keyed maps; use
`Value.uint64`, `Value.bytes`, and `Value.tensor(type_spec, :dense/:sparse/:mixed,
payload)` for explicit kinds. `Value.decode` checks the discriminator and
returns Ruby values; tensors use the `$searchkernel_tensor` tagged map.

`client.search_results(request_payload)` returns a read-only
`SearchKernel::Results::SearchResults`. Its `root` retains the recursive tree;
`hits` and `groups` are direct projections of that root. Raw `client.search`
still returns the canonical response payload. The high-level `Index` facade
and managed-local lifecycle are implemented as a separate ergonomic layer over
that canonical transport.

Run the focused tests from the repository root:

```bash
./dev test ruby
```


## Public facade

The first-class Ruby facade follows the same lifecycle shape as Python while
retaining Ruby's canonical standard-FlatBuffers-JSON transport:

```ruby
index = SearchKernel::Index.create(
  "/var/lib/searchkernel/products",
  fields: {
    "title" => SearchKernel::Text.new(stored: true, searchable: true),
    "price" => SearchKernel::FloatField.new(stored: true, filterable: true, sortable: true)
  }
)

index.insert("_id" => "shoe-1", "title" => "Running Shoes", "price" => 89.0)
results = index.search("shoes")
  .filter("price <= :max", "max" => 100.0)
  .sort("price", "asc")
  .limit(10)
  .execute
index.close
```

`Index.create` and `Index.open` acquire a managed `searchkerneld` for the index directory's parent data root; sibling managed indexes share that daemon until the last handle closes.
Set `SEARCHKERNEL_LOCAL_HOST` to override the daemon binary and
`SEARCHKERNEL_SDK_LIB` to prepend its native library directory. Startup uses a
private Unix gRPC socket plus loopback HTTP listener, probes readiness instead of
sleeping blindly, and closes the daemon process group gracefully with a hard
fallback. `Index.connect(url, target: "products")` uses an already-running HTTP endpoint without
owning its lifecycle.

Index definitions are emitted as the **standard FlatBuffers JSON representation**
of the canonical `IndexDefinition`. The daemon converts that JSON through the
official FlatBuffers parser before the existing shared IR/backend path; Ruby does
not define a second schema or semantic defaults.
