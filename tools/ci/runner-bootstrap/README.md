# SearchKernel self-hosted Actions runner bootstrap

This bundle provisions a fresh Ubuntu/Debian x86_64 machine as a SearchKernel
self-hosted GitHub Actions runner.

The host is intentionally **not** the native Vespa build environment.

Host responsibilities:

- GitHub Actions runner service
- Docker
- GitHub CLI and basic utilities
- Rust/rustfmt/clippy for host-side non-native CI
- Python + libclang for host-side protocol/ABI tooling
- Node/npm, PHP, and pinned Ruby 3.3.6 for host-side client checks
- FlatBuffers 25.12.19 for host-side protocol builds
- persistent host cache/state directories

Native SearchKernel/Vespa build and test work runs inside the persistent AlmaLinux
container created/refreshed from the checked-out repository by:

```bash
tools/actions-runner-setup.sh
```

That script owns native-base discovery, persistent Docker volumes, the Alma
toolchain, and the isolated Alma Cargo target directory. The host bootstrap does
not clone Vespa, does not install a native SDK into Ubuntu, and does not set
`VESPA_ROOT` or `SK_NATIVE_SDK`.

## Install

Use a short-lived GitHub runner registration token:

```bash
sudo ./install.sh \
  --repo https://github.com/superkelvint/searchkernel \
  --token 'REPLACE_WITH_SHORT_LIVED_TOKEN' \
  --name searchkernel-native-01
```

The runner user is added to the Docker group. The Actions service is started
after group membership is configured, so workflow jobs can invoke Docker
directly.

Ruby is cached under the persistent runner cache and reused by CI. If the host
distribution Ruby is older than the client's supported floor (Ruby 3.1), the
bootstrap and behavioral workflow use the checksum-pinned Ruby 3.3.6 source
release rather than weakening the client contract.

The bootstrap creates persistent host state under:

```text
/var/cache/searchkernel
/var/lib/searchkernel-ci
/opt/actions-runner
```

The native artifact cache is:

```text
/var/cache/searchkernel/native-artifacts
```

Workflows should provide `GH_TOKEN` from `${{ github.token }}` when invoking
repository tooling that needs private-release access. Persistent GitHub
credentials are not copied into the Alma container.

## Verify

```bash
sudo ./verify.sh
```

This verifies the runner service, Docker access for the runner user, and the
host-side non-native CI toolchain. Native correctness is verified by repository
workflows inside the Alma container.
