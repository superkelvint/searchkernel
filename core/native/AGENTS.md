# Native boundary guidance

This directory contains the Search Kernel C/C++ adapter and generated native
SDK. It is the only place where Vespa-specific native integration belongs.

## ABI and FFI

The public native ABI uses opaque handles and fixed-width primitive arguments.
Text and other repeated data cross as primitive arrays, preferably as one
structure-of-arrays transfer, never as a public aggregate struct. A future
aggregate requires an executable benchmark demonstrating that primitives are
materially inadequate; source inspection or intuition is not evidence. New
ABI additions follow the same rule.

The native ABI uses Search Kernel-owned values and opaque handles. Convert to
Vespa-specific types inside the C++ adapter. Do not expose Vespa headers,
namespaces, pointers, ownership conventions, or internal classes through the
Rust-facing API.

Keep `unsafe` in the dedicated bindings/FFI crates. The safe wrapper must make
these invariants explicit:

```text
handle ownership and exactly-once destruction
pointer/null validity
borrowed buffer/string lifetimes
array lengths/capacities
thread-safety assumptions
error payload ownership
```

Do not spread `unsafe` into ordinary API code for convenience. Catch C++
exceptions inside the adapter and translate them to the native error
representation. Rust panics must not unwind into C++. Operational failures
become Rust `Result` errors; do not abort or `unwrap()` on malformed input,
initialization failure, I/O failure, or ordinary Vespa errors.

For every value crossing the boundary, know who allocates it, who owns it, how
long borrows remain valid, who frees it, and whether calls may be concurrent.
Prefer safe, obvious copies until measurement shows that a large copy matters;
do not introduce fragile zero-copy lifetimes prematurely.

## Vespa source and generated SDK

`core/native/vespa/` is the Search Kernel adaptation layer. If the Vespa API is
awkward, adapt it there; do not spread Vespa headers or types into Rust and do
not modify upstream source.

The checkout at `upstream/vespa/` is read-only and must remain at pinned
revision:

```text
2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

Use that source and its tests as the authority for wrapped Vespa behavior; do
not guess from names or high-level documentation. When relevant, verify:

```bash
git -C upstream/vespa rev-parse HEAD
git -C upstream/vespa diff --quiet
git -C upstream/vespa diff --cached --quiet
```

If the checkout is wrong or tracked source is dirty, report it. Do not fix a
milestone by patching Vespa itself. Untracked build artifacts may exist.

`core/native/sdk/` is generated/prebuilt output. Normal Rust work consumes it
without a Vespa build or Docker session. Do not hand-edit SDK contents. Rebuild
them only when a task explicitly requires a native adapter, SDK, or otherwise
missing native capability, using repository tooling against the pinned Vespa
checkout.

Native changes must compile cleanly under the repository's configured warning
policy. Do not bypass the native adapter or replace the real Vespa path with a
mock or in-memory production backend.
