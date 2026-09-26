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
| Ruby | Deferred | implementation exists, but is not release-qualified |
| Go | Deferred | not a release surface |

Only **Linux x86-64 with glibc** is qualified. macOS, Windows, Linux ARM/aarch64,
and other operating-system/CPU combinations are explicitly unsupported for this
release. Their presence in source, generated bindings, or development tooling
must not be interpreted as a distribution promise.

The `native-full` and `client-e2e` release lanes are the executable evidence for
the supported rows. `tools/check-release-support.py` fails closed if this matrix,
README/client status, or those qualification lanes drift apart.
