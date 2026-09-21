# Native ABI validation

The native adapter is a C ABI with a deliberately small exported surface. The
version script limits the production library to the `sk_*` declarations in
`searchkernel_vespa.h`; the library is built with hidden default visibility.

The ABI artifact must retain DWARF debug information. `core/native/vespa/build-sdk.sh`
therefore adds `-O2 -g` explicitly and does not strip the adapter library.
Debug information is required because the ABI check compares the types
reachable from exported functions, not only their symbol names.

Native SDKs are environment-specific build products. They are written under
the active Cargo target root, carry a native-build fingerprint, and are tested
with a real ABI link probe before Rust uses them. See
[`../../docs/native-build-provenance.md`](../../docs/native-build-provenance.md)
for the target isolation and one-shot recovery rules.

## Compatibility contract

This section is normative for the Search Kernel native ABI.

- Existing exported functions never change signature or semantics
  incompatibly.
- Existing exported functions are never removed.
- All non-trivial native objects remain opaque handles. Their definitions stay
  private to the native adapter; the public header exposes only forward
  declarations and handle pointers.
- New capabilities normally arrive as additive:
  - new constructors;
  - new typed setters;
  - new accessors; or
  - new domain-specific opaque handles.
- Borrowed handles have an explicit parent lifetime. A function that returns or
  accepts a borrowed handle must document which parent keeps it valid and when
  the borrow ends.
- No STL or other C++ object layout crosses the ABI. Public arguments and
  results use opaque handles, fixed-width primitive values, and primitive
  arrays or byte ranges as appropriate.
- A universal type/property registry is not introduced merely to anticipate
  future capabilities.
- Native abstractions correspond to actual Vespa/searchlib/document concepts;
  the ABI does not add generic abstractions without a concrete native
  capability to wrap.
- Convenience functions may remain permanently, even after a more expressive
  underlying primitive exists. A newer primitive does not require removing or
  repurposing an older convenience function.
- Match-feature values follow Vespa's `FeatureSet::Value` boundary: the native
  representation is `DOUBLE` or opaque `DATA`, never `TENSOR`. Data returned
  through `sk_feature_value_data` is borrowed from its parent search result.
  Tensor recognition or decoding, if added later, belongs above this raw
  feature-value layer. The legacy scalar getter remains available and returns
  the dedicated type-error status when used on a DATA feature.
- Every new native capability requires both a pinned-Vespa source trace and a
  real verifier exercising the native path. Documentation or a symbol-only
  check is not sufficient evidence of support.

The following existing opaque handles are deliberately stable and must not be
redesigned as part of additive capability work:

```text
sk_engine
sk_document
sk_document_update
sk_query
sk_generic_search_request
sk_generic_search_result
```

Their opaque nature is already sufficient for future additive evolution. A
new capability should extend the surface around them or introduce another
domain-specific opaque handle when the underlying Vespa concept requires one.

## Release baseline

Do not commit `.so` files to the source repository. For each ABI freeze,
release storage must retain an unstripped baseline bundle containing at least:

```text
libsearchkernel_vespa.so
libsearchkernel_vespa.so.debug   # if debug info is split by packaging
searchkernel-vespa-engine.abi.xml
searchkernel_vespa.h
toolchain identity and target triple
```

The baseline must be associated with the ABI revision that produced it. A
comparison between different intentional ABI revisions is expected to report a
change; it must not be hidden by updating the baseline automatically.

## Comparison

After building the current unstripped library, run:

```bash
tools/check-native-abi.sh \
    "$ABI_BASELINE/libsearchkernel_vespa.so" \
    "$BUILD/libsearchkernel_vespa.so" \
    "$ARTIFACT/searchkernel-vespa-engine.abi.xml"
```

The checker invokes:

```bash
abidw --out-file "$ARTIFACT/searchkernel-vespa-engine.abi.xml" "$BUILD/libsearchkernel_vespa.so"
abidiff --fail-no-debug-info \
    "$ABI_BASELINE/libsearchkernel_vespa.so" \
    "$BUILD/libsearchkernel_vespa.so"
```

Missing `abidiff`, `abidw`, either library, or usable debug information is a
failure. The ABIXML snapshot is written before `abidiff`, so an intentional
change still leaves a reviewable description even though the comparison exits
nonzero.

The checker’s invocation and failure propagation can be exercised without
libabigail by running:

```bash
tools/test-check-native-abi.sh
```

An ABI change requires explicit review. In particular, changing or removing
an existing exported signature or reachable type requires a native ABI version
bump and a new baseline. Compatible additions still require review because
they change the exported contract and the version-script symbol set.

## Public header aggregate check

The public header is also checked before native ABI comparison. The check uses
libclang to inspect each function declaration and rejects pointers to, or
returns of, fully defined `struct`/`union` types. Forward declarations remain
opaque and pass. An exception must be added to `ALLOWLIST` in
`tools/check_abi_surface.py` with a benchmark result and review/PR reference.

The CI job installs `python3-clang` and `libclang` and runs:

```bash
python3 tools/check_abi_surface.py core/native/vespa/searchkernel_vespa.h
```

The focused regression suite can be run locally with:

```bash
python3 tools/test_check_abi_surface.py
```
