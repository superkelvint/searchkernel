# M1 cache/worktree performance evidence

Environment: canonical `searchkernel-ci` Docker execution on the M1 development host.
The source container is recreated for every `./dev` invocation. Immutable Vespa and
sccache inputs remain shared by immutable identity.

## Pre-change baseline

`origin/main` at `2dfe376fc133a64186bd69d93aea025ea616fa7b` used a new random
`/cargo-target` Docker volume for every invocation. Two unchanged fresh-container
`./dev test ruby` runs measured:

| run | elapsed | Rust `Compiling`/`Checking` lines |
| --- | ---: | ---: |
| 1 | 125.79 s | 112 |
| 2 | 124.47 s | 112 |

The second invocation therefore behaved like a clean build and violated the M1
warm-container acceptance requirement.

## Converged ownership

Writable Cargo/native state now uses a persistent Docker volume keyed by the
canonical host worktree path plus the leaf image's immutable environment ID.
Different worktrees or environment identities receive different writable volumes;
CI job state is unchanged. Source checkout mtimes are normalized to the commit
before the working-tree overlay is applied so a fresh source container does not
make unchanged sources spuriously newer than the persistent target tree.

After warming the changed build scripts, two unchanged fresh-container Ruby runs
were used to validate the final state. The acceptance run measured 37.84 s with
**zero** Rust `Compiling`/`Checking` lines, zero native SDK/Vespa preparation,
zero consumer-runtime download, and zero generated-code regeneration. Ruby unit,
gem/package, and managed-local E2E tests still executed.

## First-class language fresh-container benchmark

The checked-in lockfile requires the refreshed toolchain identity
`d9185e24a613a97250ed550884190132215f2ebc5eb839be0d8b29cb9e68091c`.
Docker Hub credentials on the development host could build but not push that
image (`denied: requested access to the resource is denied`), so the benchmark
used a locally built thin leaf with that exact toolchain plus the canonical
immutable Vespa digest. This is an infrastructure publication credential issue,
not a test bypass; the same leaf Dockerfile and runtime path were used.

| scope | fresh #1 | Rust compile lines | fresh #2 | Rust compile lines |
| --- | ---: | ---: | ---: | ---: |
| `ir` | 29.21 s | 21 | 6.08 s | 0 |
| `rust-api` | 38.87 s | 11 | 6.62 s | 0 |
| `python` | 129.20 s | 88 | 33.52 s | 0 |
| `typescript` | 80.65 s | 2 | 79.86 s | 2 |
| `php` | 23.10 s | 0 | 23.19 s | 0 |
| `ruby` | 47.13 s | 1 | 37.85 s | 0 |

TypeScript's two Rust compile lines are the protocol/daemon E2E build and remain
a lane-local optimization opportunity; the second-run wall time is dominated by
TypeScript generation/package/test/E2E work rather than a clean Rust rebuild.
The required Ruby warm recreation assertion is satisfied: its second independent
container performs zero Rust compilation and the log also contains zero native
SDK/Vespa preparation, consumer-runtime download, or generated-code regeneration.

During this benchmark `./dev test php` exposed that the PHP real-engine fixture
invoked its Python index preparer without making the in-tree Python client
importable. The lane now supplies the repository Python client through
`PYTHONPATH`; both fresh-container PHP runs pass.
