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
and managed local lifecycle remain separate future slices.

Run the focused tests from the repository root:

```bash
./dev test ruby
```
