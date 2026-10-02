# Release notes

## 0.1.x beta

Search Kernel 0.1.x beta is qualified on Linux x86-64 with glibc for Rust embedded, searchkerneld, Python managed-local and connect, TypeScript/Node connect, and Ruby connect. Browser gRPC-Web, PHP, Ruby managed-local, Go, macOS, Windows, and Linux aarch64 are deferred. See the [release support and stability contract](release-support.md).

The Python release wheel uses the `manylinux_2_28_x86_64` platform tag. Python requires 3.10 or newer and Ruby requires 3.1 or newer. Cargo metadata uses edition 2021 and does not declare `rust-version`; the Node.js package metadata does not declare a minimum Node.js version.

The [installation guide](getting-started.md) documents release artifacts, daemon ownership, and troubleshooting. The [generated release quickstarts](release-quickstart.md) are the canonical executable examples used by clean-consumer qualification. Their current coverage gaps and a Node endpoint default mismatch are tracked in [quickstart follow-up #1019](https://github.com/superkelvint/searchkernel/issues/1019).

Known feature limitations are listed in [LIMITATIONS.md](../LIMITATIONS.md). Select release artifacts and checksums from the same manifest.
