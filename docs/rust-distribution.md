# Rust consumer distribution

The Cargo product is embedded search by default:

```toml
[dependencies]
searchkernel = "0.1"
```

Remote-only applications explicitly opt out:

```toml
[dependencies]
searchkernel = { version = "0.1", default-features = false, features = ["remote"] }
```

The release producer packages normalized `.crate` candidates for the public crate
and its first-party dependencies. It does not distribute a workspace tarball or
require a consumer to use a repository path dependency. Registry publication is
a separate release action; producing a candidate does not publish it.

Each released bindings crate contains an exact `runtime.json` descriptor:
Search Kernel version/revision, package generation, platform/target, build
environment, native ABI, Vespa revision, runtime identity, immutable artifact URL,
archive SHA-256 and payload SHA-256. The first native Cargo build acquires the
qualified **consumer runtime**, using the shared runtime cache contract. It does
not compile the Vespa adapter or fetch a developer native-base SDK.

The supported native target remains Linux x86_64 glibc. Cross-compilation and
packaged maintainer hardening builds fail explicitly. A packaged native build
rejects `SK_NATIVE_SDK` and `SK_NATIVE_ABI_HEADER` rather than using them to bypass
the package identity. The source checkout retains its contributor SDK workflow.

Runtime acquisition uses an inter-process file lock, archive checksum validation,
safe extraction, identity/payload validation and atomic cache publication. A warm
valid cache performs no acquisition. Operators may configure the shared
`SEARCHKERNEL_RUNTIME_CACHE`, mirror the immutable artifact through
`SEARCHKERNEL_RUNTIME_ORIGIN`, or provide an already-qualified
`SEARCHKERNEL_RUNTIME_DIR`; none changes the expected identity/digests. Building
the application ahead of deployment prewarms the same runtime cache. Normal
installation requires no runtime overrides or Python interpreter.

The adapter's ELF SONAME names its full runtime identity beneath
`$ORIGIN/.searchkernel-native/`. Cargo stages the private library closure beside
normal, test and example executable directories. Different generations have
different paths. Existing mismatched sidecars fail closed. The build checks the
actual SONAME and links **and executes** a probe against the Rust ABI constant.
Deploy the executable together with its identity directory; moving both preserves
the relative loader contract without `LD_LIBRARY_PATH` or repository paths.

The pure Rust FlatBuffers protocol does not require the optional C++ JSON bridge.
That bridge remains enabled by default for direct protocol users and daemon HTTP
support; native/remote facade dependencies select binary protocol support without
installing a FlatBuffers C++ toolchain. The explicit protocol `json` feature keeps
its existing pinned FlatBuffers contributor prerequisite.

## Qualification

All commands use the canonical `./dev` container path:

```bash
./dev test cargo-runtime
./dev test consumer-release-policy
./dev test consumer-release
./dev ci
```

The runtime tests exercise the shared identity vector, concurrent cold acquisition,
warm reuse, archive/override corruption, target/ABI rejection, unsafe extraction,
generation-specific sidecars and real ELF SONAME validation. Package/registry
tests cover immutable staging, failure cleanup, dependency aliases, features,
checksums and serving the exact candidate bytes.

The complete release lane creates a fresh external Cargo project and fresh Cargo
home. A temporary sparse registry serves exactly the release `.crate` files and
proxies third-party crates. Missing Search Kernel crates never fall back to
published packages. The consumer uses `cargo add searchkernel` and default
features, then builds, executes directly, verifies native insert/search/reopen,
relocates the executable/runtime, and builds again offline in a new target
directory using the warm runtime cache. A second consumer verifies the explicit
remote-only opt-out without provisioning native assets. No developer native-base fixture is
mounted or injected. Other ecosystem qualification remains unchanged.

`./dev test cargo-distribution-generation` explicitly regenerates the Cargo
lockfile, Rust formatting and public quickstart documentation. Its generated
diff must be reviewed and committed before verification. Candidate packaging uses
the checked-in lockfile and stages all release-only files in a temporary workspace;
the checkout and its generated native source bundle remain untouched.

These commands define acceptance; source review or a successful workspace build
alone does not establish distributable-Cargo qualification.
