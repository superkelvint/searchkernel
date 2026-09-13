# M0 contract — native SDK

## Goal

Package the proven embedded Vespa engine behind a private Search Kernel native
ABI that can be consumed outside the Vespa development container.

## Required SDK layout

```text
$SK_NATIVE_SDK/
    include/searchkernel_vespa.h
    lib/libsearchkernel_vespa.so
    share/embedded_crud_cfg/...
    manifest.json
```

`manifest.json` must contain at least:

```json
{
  "native_abi_version": 2,
  "vespa_commit": "2300ef0412bdb76f227ae3b497c8ca5e73c5da4e"
}
```

The exported M0 ABI must match `NATIVE_ABI_CONTRACT.h` for:

```text
sk_native_abi_version
sk_engine_open
sk_engine_destroy
```

## Acceptance

A C program compiled outside the Vespa source tree must:

1. dynamically load/link `libsearchkernel_vespa.so`;
2. report native ABI version 2;
3. open a fresh engine using the bundled config fixture;
4. destroy it cleanly;
5. repeat open/destroy on the same data directory;
6. exit successfully.

`ldd` must report no missing libraries.

## Does not count

- running the old gtest executable directly;
- requiring the Vespa source checkout at runtime;
- requiring the Vespa development container for the acceptance command;
- exposing Vespa headers through `searchkernel_vespa.h`;
- spawning a full Vespa deployment.
