# Vespa Pin Upgrade SOP

This procedure is for a Codex agent upgrading the Vespa source used by the
Search Kernel native adapter. It is a qualification workflow, not ordinary
dependency maintenance.

The agent must discover the candidate release, qualify it in disposable state,
and only then update the repository pin. A failed qualification leaves the
existing pin in place.

## Safety invariants

- Never edit tracked files below upstream/vespa/ in place.
- Never run a candidate build in the active worktree.
- Never change a verifier, contract, fixture, golden, comparator, or expected
  behavior to make a candidate pass.
- Keep candidate checkouts, build trees, SDKs, logs, and runtime data below
  target/vespa-upgrade/<run-id>/.
- Treat the current pin and preserved persistent fixture as the rollback
  baseline.
- Do not update the canonical pin until every mandatory gate and the audit
  have completed.

If the active worktree is dirty, preserve it. Use a clean copy of HEAD for
qualification and do not apply uncommitted changes unless the user explicitly
requests qualification of those changes.

Before starting any delegated or long-running work, hash the active status:

~~~
git status --porcelain=v1 > "$RUN/active-status.before"
sha256sum "$RUN/active-status.before" > "$RUN/active-status.before.sha256"
~~~

Recheck this before any canonical update. If another actor changes the active
worktree during qualification, invalidate the run for update purposes. Do not
restore, overwrite, or clean up those concurrent changes.

## 1. Discover released candidates

Read the current pin from:

~~~
reference/vespa/PINNED_VESPA.json
~~~

Record:

~~~
git -C upstream/vespa rev-parse HEAD
git -C upstream/vespa describe --tags --always
git -C upstream/vespa diff --quiet
git -C upstream/vespa diff --cached --quiet
~~~

Use the official vespa-engine/vespa repository as the release source:

~~~
git ls-remote --tags --refs https://github.com/vespa-engine/vespa.git
~~~

Keep stable tags matching vMAJOR.MINOR.PATCH. For each candidate, resolve
and record its version, tag-object SHA, peeled commit SHA, GitHub release
title/date, draft/prerelease state, tag and commit signature status, and source
metadata identifying the Vespa engine version.

A candidate is eligible only if it is newer than the nearest stable release
associated with the current pin, is not a draft or prerelease, and has
verifiable official provenance. Do not infer engine compatibility from a CLI
release title. The supplied v8.753.16 page is titled “Vespa CLI 8.753.16”;
its source commit must still be checked as the embedded-engine source before
adoption:

~~~
tag object: 66e1a3c32ebba9dfea5f6e054078d0d2dfc9893b
commit:     42bfc5f6ef7a281d560439aec074613a2063e959
~~~

Verify the selected tag in a temporary clone:

~~~
git clone --no-checkout https://github.com/vespa-engine/vespa.git \
  target/vespa-upgrade/<run-id>/vespa-source
git -C target/vespa-upgrade/<run-id>/vespa-source fetch --tags --force origin <tag>
git -C target/vespa-upgrade/<run-id>/vespa-source verify-tag <tag>
git -C target/vespa-upgrade/<run-id>/vespa-source rev-parse <tag>^{commit}
~~~

If signature verification or engine-release provenance fails, stop and report
the candidate as unqualified.

GitHub's verified badge and a locally parsed signature are useful provenance,
but adoption also requires a locally trusted signer. Configure
`gpg.ssh.allowedSignersFile` in the disposable clone and record the matching
principal/key fingerprint. A signature with no trusted local principal is a
provenance blocker, not a pass.

If there are no eligible releases, report that the repository is current and
make no changes. If there is one eligible release, select it automatically.
If there are multiple eligible releases, show them newest first and ask the
user which one to qualify, with the newest preselected. In unattended mode,
use the newest eligible release and record that default decision.

Record the complete discovery and selection in run-manifest.json:

~~~json
{
  "current_commit": "...",
  "current_release": "...",
  "candidates": [
    {"version": "...", "tag": "...", "commit": "...", "signed": true}
  ],
  "selected_version": "...",
  "selected_tag": "...",
  "selected_commit": "...",
  "selection_reason": "user|only-candidate|latest-default"
}
~~~

## 2. Capture the baseline

Create a clean temporary repository copy from the current HEAD. Do not qualify
from the active dirty worktree. Record:

~~~
git rev-parse HEAD
git -C upstream/vespa rev-parse HEAD
cat reference/vespa/PINNED_VESPA.json
cat core/native/sdk/linux-x86_64/manifest.json
~~~

Run the environment preflight before any native qualification:

~~~
command -v cmake c++ llvm-config python3 node npm
find /usr /opt -name LLVMConfig.cmake -o -name llvm-config 2>/dev/null
python3 -c 'import pytest; import clang.cindex'
git config --get gpg.ssh.allowedSignersFile
~~~

The candidate Vespa CMake configure must have a usable LLVM development
installation, including `LLVMConfig.cmake` or `llvm-config`, before compilation
is attempted. Python `pytest` and `libclang` are required for the Python and
ABI-surface lanes. Missing prerequisites are recorded as blockers and are not
silently skipped.

A clean Git clone does not contain the generated SDK. Before running native
baseline tests, either copy or rebuild the current pinned SDK into the run
directory, then assert that its manifest commit equals the current pin and that
its library/header/configuration files exist. Use absolute SDK and Cargo
target paths; do not rely on relative paths after changing directories. If the
baseline SDK cannot be made valid, mark native baseline results blocked rather
than producing cascading linker failures.

Run the baseline:

~~~
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
./reference/vespa/verify-through.sh m11
./reference/vespa/verify-through.sh i
./reference/api-conformance/verify.sh
./reference/clients/c2/verify-c2.sh
python3 -m pytest clients/python/tests
(cd searchkernel-web && npm ci && npm test && npm run build)
~~~

Missing tooling is a failure, not a reason to silently skip a test. Record the
exact command, environment, exit status, and artifact paths for every test.
Preserve the current M5 fixture and metadata as the mandatory migration input.

## 3. Build the candidate in disposable state

Use a fresh candidate Vespa checkout and clean build tree. Do not reuse the
current Vespa CMake build tree: its generated flags, libraries, and absolute
paths belong to the old revision.

Use a pinned Vespa development image, not an unrecorded latest image. Record
the immutable image digest, compiler versions, dependency installation, target
triple, and build settings.

Run candidate CMake configure as a distinct gate before the SDK build. If it
cannot locate LLVM or another required dependency, stop candidate
qualification, record the exact configure log, and do not run native tests
against the old SDK as a substitute.

Configure and build the candidate using its upstream build instructions. The
build must provide the configured target files consumed by:

~~~
core/native/vespa/build-sdk.sh
~~~

Build an SDK in the run directory:

~~~
VESPA_ROOT="$RUN/vespa-source" \
SK_NATIVE_SDK="$RUN/sdk/linux-x86_64" \
SK_NATIVE_BUILD_DIR="$RUN/native-build" \
  core/native/vespa/build-sdk.sh
~~~

Validate that the SDK manifest commit matches the selected commit, the public
header leaks no Vespa types, symbols satisfy the ABI policy, dynamic
dependencies resolve, generated bindings match the header, and any ABI change
has explicit review.

A temporary candidate-pin overlay may update only pin/provenance metadata so
the existing verifiers can identify the candidate. Behavioral scenarios,
expected search results, contracts, comparators, and goldens remain unchanged.

## 4. Run compatibility qualification

Run the full test matrix against the candidate SDK and Vespa source. It must
cover:

- native ABI, lifecycle, dependency, and symbol checks;
- M0 through M11 cumulative verification;
- product-path, API, and client conformance;
- linguistics and Java-replacement parity;
- schema/configuration generation;
- put, update, delete, reopen, text matching, phrases, filters, attributes,
  docsums, ranking, grouping, and result ordering;
- repeated open/close and failed-initialization recovery;
- sanitizer, stress, resource, and dependency checks;
- persistent recovery and reopen behavior.

Use the assertion policy in the End-to-End Qualification Specification:
EXACT values remain identical; SEMANTIC values retain equivalent meaning;
ORDERED results retain membership and contractual order; BOUNDED values remain
within approved limits; OBSERVATIONAL differences are recorded only.

Mandatory migration:

~~~
current Vespa revision
    -> preserved M5 persistent fixture
    -> candidate Vespa revision
    -> reopen, search, mutate, close, reopen, verify
~~~

Passing only newly-created-index tests is insufficient.

## 5. Audit the run

The execution agent performs a separate audit before recommending adoption:

~~~
git -C "$ACTIVE_REPO" status --short
git -C "$ACTIVE_REPO" diff -- upstream/vespa
git -C "$ACTIVE_REPO" diff -- reference/
find "$RUN" -maxdepth 3 -type f -print
~~~

The audit must establish that the active worktree was not unexpectedly changed,
tracked upstream source was not edited, the candidate SDK matches the selected
commit, every mandatory test ran, no fixture or golden was weakened, the
persistent fixture originated from the old revision, and every warning or
failure has an explicit disposition.

Update this SOP after the run if any command, prerequisite, discovery rule, or
audit check was incomplete or misleading.

## 6. Apply the update only after qualification passes

1. Preserve old commit, pin manifest, and SDK manifest in the run report.
2. Replace the canonical Vespa checkout with a clean checkout of the selected
   commit; carry no local modifications into it.
3. Update all allow-listed pin and provenance references.
4. Rebuild the SDK from selected source; never hand-edit generated output.
5. Run the complete test matrix again on the canonical updated repository.
6. Review the final diff and manifests.

If any post-update test fails, restore the old exact checkout and pin metadata,
retain the failure report, and do not declare the upgrade complete.

## Completion criteria

The upgrade is complete only when a signed exact candidate was selected,
multiple newer choices were shown when applicable, sandbox qualification passed
before the canonical update, the complete test matrix passed before and after
the update, old-index migration passed, upstream source is clean at the
selected commit, no acceptance artifact was weakened, evidence is retained,
and rollback remains possible.
