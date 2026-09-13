# Native ABI validation

The native adapter is a C ABI with a deliberately small exported surface. The
version script limits the production library to the `sk_*` declarations in
`searchkernel_vespa.h`; the library is built with hidden default visibility.

The ABI artifact must retain DWARF debug information. `native/vespa/build-sdk.sh`
therefore adds `-O2 -g` explicitly and does not strip the adapter library.
Debug information is required because the ABI check compares the types
reachable from exported functions, not only their symbol names.

## Release baseline

Do not commit `.so` files to the source repository. For each ABI freeze,
release storage must retain an unstripped baseline bundle containing at least:

```text
libsearchkernel_vespa.so
libsearchkernel_vespa.so.debug   # if debug info is split by packaging
searchkernel-vespa.abi.xml
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
    "$ARTIFACT/searchkernel-vespa.abi.xml"
```

The checker invokes:

```bash
abidw --out-file "$ARTIFACT/searchkernel-vespa.abi.xml" "$BUILD/libsearchkernel_vespa.so"
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
