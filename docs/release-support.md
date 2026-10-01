# Release support matrix

This document describes the **distribution promise** for the next Search Kernel
beta. Implementation present in the repository is not automatically a supported
release surface. The machine-readable authority is [`release-support.json`](release-support.json).

| Surface | Linux x86-64 glibc | Release mode |
| --- | --- | --- |
| Rust embedded API | Supported | in-process native engine |
| `searchkerneld` | Supported | native daemon |
| Python | Supported | managed-local and connect-to-daemon |
| TypeScript / Node.js | Supported | managed-local and connect-to-daemon |
| TypeScript in browsers | Deferred | gRPC-Web implementation exists, but is not release-qualified |
| PHP | Deferred | implementation exists, but is not release-qualified |
| Ruby | Supported | managed-local and connect-to-daemon |
| Go | Deferred | not a release surface |

Only **Linux x86-64 with glibc** is qualified. macOS, Windows, Linux ARM/aarch64,
and other operating-system/CPU combinations are explicitly unsupported for this
release. Their presence in source, generated bindings, or development tooling
must not be interpreted as a distribution promise.

Release qualification is invoked through the canonical `./dev` test/verify
targets recorded in `release-support.json` and registered in
`build/targets.json`. `tools/check-release-support.py` validates those live
repository commands directly; it does not treat retired/reference GitHub Actions
YAML as executable evidence.

Ruby's managed-local and connect-to-daemon qualifications use the installed gem
against the same immutable consumer runtime identity as the Python and Node
packages. The managed-local path lazily acquires that runtime through the
shared consumer cache.

## Consumer quickstarts

The canonical executable release quickstarts are rendered in [release-quickstart.md](release-quickstart.md) and are executed by the clean-consumer qualification gate.

## Python / Ruby parity boundary

Issue #803's final adversarial pass compares the release-relevant ergonomic
behavior of the Python and Ruby clients, not whether the two implementations use
identical language constructs or wire plumbing. Managed-local Ruby behavior is
also covered by the consumer package qualification sequence.

| Area | Classification | Release meaning |
| --- | --- | --- |
| Search, typed queries, filtering, sorting, pagination, presentation, grouping, ranking, tensors, document mutation, analysis, inspect/describe, persistence/reopen, result decoding, errors, and close semantics | Supported parity | Ruby has executable real-engine coverage for the Python public behavior surface. Absent result collections and counters are normalized to the same stable empty/zero defaults. Both managed-local and connect-to-daemon package paths are release-qualified. |
| Daemon transport | Intentional non-goal | Python uses generated gRPC/FlatBuffer plumbing; Ruby uses the sanctioned BFBS-backed standard-FlatBuffers-JSON HTTP edge. Both enter the same canonical dispatcher and are qualified for connect-to-daemon behavior. |
| Low-level client representation | Intentional non-goal | Python exposes generated FlatBuffer object types while Ruby exposes idiomatic Hash/JSON helpers. Release parity is defined at the ergonomic/public behavior boundary, not object identity across languages. |
| Idiomatic result wrappers | Intentional non-goal | Where the canonical response carries the same information, Python may return generated result objects while Ruby returns scalar IDs/booleans or decoded Hash/Array values. Parity requires the same observable operation outcome and error semantics, not identical language-level wrapper types. |
| Error presentation | Intentional non-goal | Python presents canonical error codes as snake_case names while Ruby preserves the canonical enum spelling and has a distinct transport-error subclass. The canonical error category, message/details, retryability, and failure behavior remain equivalent. |
| Close failure cleanup | Supported parity | Owned-vs-external daemon lifecycle behavior is exercised as parity evidence. Ruby additionally guarantees managed-owner cleanup if closing the client transport itself raises. |
| Lifecycle syntax | Intentional non-goal | Python additionally offers context-manager syntax; Ruby uses explicit `close` with normal Ruby `ensure` patterns. |
| Additive Ruby helpers | Intentional non-goal | Ruby exposes some additional query/schema/search conveniences. Python is not required to mirror Ruby-only sugar, and those additions do not weaken Python-to-Ruby parity. |
| Grouping continuation request/resume semantics | Backend limitation | Continuation values can be decoded, but embedded continuation/cursor execution remains deferred for all clients. |
| Conditional native mutations | Backend limitation | The public condition surface is preserved; the current native backend returns typed `Unsupported` where conditional execution is unavailable. |
| Native sort-value qualification | Backend limitation | Result decoders preserve sort values when present, but the current native ABI does not expose enough data to require a real-engine sort-value readback assertion. |
| Package format | Intentional non-goal | Python ships as a Python package/wheel and Ruby as a gem. Both packages obtain the qualified native runtime without repository-local state or an injected `SEARCHKERNEL_LOCAL_HOST`. |
