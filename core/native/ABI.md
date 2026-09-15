# Native ABI validation

The native adapter is a C ABI with a deliberately small exported surface. The
version script limits the production library to the `sk_*` declarations in
`searchkernel_vespa.h`; the library is built with hidden default visibility.

The ABI artifact must retain DWARF debug information. `core/native/vespa/build-sdk.sh`
therefore adds `-O2 -g` explicitly and does not strip the adapter library.
Debug information is required because the ABI check compares the types
reachable from exported functions, not only their symbol names.

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
