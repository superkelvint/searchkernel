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

The current foundation exposes the complete low-level operation envelope,
portable/protocol error separation, and semantic-free HTTP transport. Higher
level values, results, `Index` ergonomics, and managed local lifecycle are added
in subsequent bounded slices under the full Ruby client tracker.

Run the focused tests from the repository root:

```bash
./dev test ruby
```
