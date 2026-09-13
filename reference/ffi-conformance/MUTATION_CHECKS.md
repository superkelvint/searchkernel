# Mutation expectations

The verifier is intentionally structured so the following implementation mutations must fail:

| Mutation | Expected failing verifier |
|---|---|
| Add `sk_search` or another per-operation export | `check-symbols.sh` / `check-source-invariants.sh` |
| Change a numeric ABI status | C smoke / Rust verifier |
| Add normal dependency on Vespa | `check-dependencies.sh` |
| Add normal dependency on `searchkernel-testkit` | `check-dependencies.sh` |
| Use unchecked FlatBuffers root parsing | `check-source-invariants.sh` |
| Bypass semantic validation for an invalid search field | semantic-error test |
| Route an operation to the wrong `*_to_ir` path | exact C3 `ScriptedBackend` request check |
| Convert a `PortableError` to non-zero C status | semantic-error test |
| Mutate caller request memory | operation round-trip request equality |
| Return output on an ABI failure | malformed/protocol/pointer tests |
| Make `sk_buffer_free` a no-op | live-buffer counter and repeated-cycle tests |
| Key buffers only by address | allocation-token ownership regression |
| Free by pointer without validating the allocation token | allocation-token ownership regression |
| Reuse closed handle values | handle lifecycle test |
| Let a call start after close | concurrent-close test |
| Cancel an already-started call on close | concurrent-close test |
| Hold the registry mutex during backend execution | concurrent-close test deadlocks/fails |
| Allow panic to unwind through C | panic-containment test/process failure |
| Accept response bytes as request bytes | response-as-request test |
| Accept a truncated FlatBuffer prefix | truncation test |

These mutation expectations are part of C5 acceptance. An implementation should not weaken the verifier to accommodate a mutation.
