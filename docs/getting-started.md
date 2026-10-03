# Install and run Search Kernel

This guide covers the currently qualified 0.1.x beta consumer surfaces. Use the [generated release quickstarts](release-quickstart.md) for executable Rust, Python, TypeScript/Node, and Ruby examples; clean-consumer qualification runs those same canonical sources. All four quickstarts define the same schema, insert the same three documents, search, filter, sort, close, reopen the persistent index or target, and verify the results.

## Supported platform and surfaces

The qualified platform is Linux x86-64 with glibc. The beta supports Rust embedded, the packaged `searchkerneld` daemon, Python managed-local and connect, TypeScript/Node connect, and Ruby connect. Browser gRPC-Web, PHP, Ruby managed-local, Go, macOS, Windows, and Linux aarch64 are deferred. See the [release support matrix](release-support.md).

## Release artifacts

Use artifacts from one release together. The release `manifest.json` lists each artifact filename and SHA-256 checksum; select the exact runtime and package artifacts there.

| Surface | Artifact | Runtime requirement |
| --- | --- | --- |
| Rust embedded | `searchkernel-rust-0.1.0.tar.gz` source bundle | Workspace edition is 2021; no explicit `rust-version` is declared |
| `searchkerneld` | `searchkernel-runtime-<runtime-identity>-linux-x86_64-glibc.tar.gz` | Linux x86-64 with glibc |
| Python | Wheel tagged `manylinux_2_28_x86_64` | Python 3.10 or newer |
| TypeScript / Node.js | `@searchkernel/client` version 0.1.0 | Package metadata does not declare a minimum Node.js version |
| Ruby | `searchkernel-0.1.0.gem` | Ruby 3.1 or newer |

The complete Python wheel filename is release-specific. Select it from the release manifest; the supported platform tag is `manylinux_2_28_x86_64`.

## Installation and runtime ownership

### Rust embedded

The Rust artifact is a source bundle containing the workspace and Rust crates. Use the [canonical Rust embedded quickstart manifest and program](release-quickstart.md#rust-embedded-manifest). Its current build provisions the pinned native runtime from the release service; when the runtime release is in a private repository, authenticate with the GitHub CLI or set `GH_TOKEN`. The workspace uses edition 2021 and does not declare an explicit `rust-version`.

Rust embedded runs in the application process. The application owns its engine handles and persistent index paths; closing and reopening the same path demonstrates persistence in the quickstart.

### Python

Set `PYTHON_WHEEL` to the wheel filename selected from the release manifest, then install it with `python -m pip install "$PYTHON_WHEEL"`. Use Python 3.10 or newer. The wheel carries the packaged daemon and native runtime used by managed-local mode. In that mode the Python client starts and manages a local daemon host; index data persists at the configured local path. In connect mode, the Python client connects to a separately managed `searchkerneld` gRPC listener and does not own the daemon process.

See the [Python managed-local quickstart](release-quickstart.md#python-managed-local). Python connect is a supported mode; use the daemon's `--listen` address for its gRPC endpoint.

### TypeScript / Node.js

Set `NODE_PACKAGE` to the package archive selected from the release manifest, then install it with `npm install "$NODE_PACKAGE"`. The package is `@searchkernel/client` 0.1.0. Its package metadata does not declare a minimum Node.js version. The supported release mode is Node.js connect over gRPC to a separately managed `searchkerneld` listener configured with `--listen`. The client does not own the daemon or its persistent data root.

See the [TypeScript / Node connect quickstart](release-quickstart.md#typescript--node-connect). It connects over gRPC to the daemon's `--listen` address and defaults `SEARCHKERNEL_ENDPOINT` to `127.0.0.1:50051`; set the variable when using a different address.

### Ruby

Install the `searchkernel-0.1.0.gem` artifact selected from the release manifest with `gem install ./searchkernel-0.1.0.gem`. Ruby 3.1 or newer is required. The supported Ruby mode connects to a separately managed daemon through its optional HTTP/1 listener, configured with `--web-listen`. The Ruby client does not own the daemon process or data root.

See the [Ruby connect quickstart](release-quickstart.md#ruby-connect). Start the daemon with `--web-listen` enabled and set `SEARCHKERNEL_ENDPOINT` to that listener's HTTP endpoint.

### Standalone searchkerneld

The runtime archive includes the packaged daemon and native runtime. Start one daemon with a data root; it owns physical index locations beneath that root and serves multiple logical indexes. Clients create or open logical indexes and select their target.

Set `RUNTIME_ARCHIVE` to the runtime archive selected from the release manifest, then run:

~~~bash
mkdir -p searchkerneld-runtime
tar -xzf "$RUNTIME_ARCHIVE" -C searchkerneld-runtime
./searchkerneld-runtime/bin/searchkerneld --data-root ./search-data --listen 127.0.0.1:50051 --web-listen 127.0.0.1:50052
~~~

The `--listen` address serves gRPC for Python and TypeScript/Node connect clients. Ruby uses the optional HTTP/1 listener configured by `--web-listen`. On Unix, omitting `--listen` selects the default Unix-domain socket at `/tmp/searchkerneld.sock`. TCP listeners are for clients in other processes or hosts.

## Persistence and reopen behavior

Rust embedded and Python managed-local store data at the local index path and can reopen that same path after close. The standalone daemon owns data beneath `--data-root`; a client selects a logical index target, and the daemon manages its physical location. Connect clients closing their connection does not transfer ownership of the daemon or its indexes. Use the same local path, or the same daemon data root and logical target, when reopening an index.

The Rust and Python quickstarts reopen the same local index path. The TypeScript/Node and Ruby quickstarts create a daemon target, close the client index, and reopen that same target. Each quickstart verifies its search results after reopening.

## Troubleshooting

- **Wheel rejected by pip:** use the Linux x86-64 glibc wheel tagged `manylinux_2_28_x86_64` and Python 3.10 or newer. The manifest provides the complete wheel filename.
- **Daemon connection fails:** confirm the daemon is running and the client endpoint matches its listener. Python and Node use `--listen` for gRPC; Ruby uses `--web-listen` for HTTP/1.
- **Node quickstart cannot connect:** confirm the daemon exposes gRPC at `127.0.0.1:50051`, or set `SEARCHKERNEL_ENDPOINT` to the address configured by `--listen`.
- **Index does not reopen:** use the same local path for embedded or managed-local mode, or the same daemon data root and logical target in connect mode. Do not remove the stored index data between close and reopen.
- **Rust runtime download fails:** authenticate with the GitHub CLI or set `GH_TOKEN` when the release service requires access to a private repository.
- **A package or platform is missing:** check the [release support matrix](release-support.md). The deferred surfaces are not qualified for this beta.

See [LIMITATIONS.md](../LIMITATIONS.md) for advertised feature limitations and [BUILDING.md](../BUILDING.md) for contributor setup and source builds.
