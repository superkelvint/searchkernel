#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
DEV="$ROOT/dev"
RUNNER="$ROOT/tools/run-in-ci-image.sh"
RUNTIME="$ROOT/tools/ci-runtime-ownership.sh"
COVERAGE="$ROOT/tools/coverage.sh"
WORKFLOW="$ROOT/.github/workflows/on-demand-verification.yml"

bash -n "$DEV"
bash -n "$RUNNER"
bash -n "$RUNTIME"
bash -n "$COVERAGE"
python3 "$ROOT/tools/test_dev_client_e2e.py"

python3 - "$DEV" "$RUNNER" "$WORKFLOW" <<'PY'
from pathlib import Path
import sys

dev = Path(sys.argv[1]).read_text(encoding="utf-8")
runner = Path(sys.argv[2]).read_text(encoding="utf-8")
workflow = Path(sys.argv[3]).read_text(encoding="utf-8")

dispatch = 'exec bash "$ROOT/tools/run-in-ci-image.sh" "$@"'
resolve_python = 'PYTHON=$(bash "$ROOT/tools/resolve-python.sh")'
if dispatch not in dev:
    raise SystemExit("dev does not dispatch build/test/verify commands through the CI image")
if dev.index(dispatch) > dev.index(resolve_python):
    raise SystemExit("dev must dispatch before resolving any host language toolchain")
for command in (
    "check|galaxy|universe|check-fast|check-protocol|coverage|lint|test|verify",
    "native-sdk)",
    "native-base)",
    '[[ "${2:-ensure}" == "ensure" ]]',
    "SEARCHKERNEL_DEV_USE_CI_IMAGE",
    "SEARCHKERNEL_IN_CI_IMAGE",
    "/opt/searchkernel/native-validation-pass",
):
    if command not in dev:
        raise SystemExit(f"dev image-dispatch contract is missing {command!r}")

native_dispatch = dev.index("native-sdk)")
native_case_end = dev.index(";;", native_dispatch)
native_case = dev[native_dispatch:native_case_end]
if '"ensure"' not in native_case:
    raise SystemExit("dev must dispatch native-sdk ensure through the CI image")
if '"path"' in native_case:
    raise SystemExit("dev must keep native-sdk path host-local; container paths are not host-usable")

coverage_start = dev.index("coverage_target() {")
coverage_end = dev.index("\nlint_target()", coverage_start)
coverage_body = dev[coverage_start:coverage_end]
if "native_sdk_command ensure >/dev/null" not in coverage_body:
    raise SystemExit("dev coverage must provision the native SDK before all-features tests")
if coverage_body.index("native_sdk_command ensure >/dev/null") > coverage_body.index('bash "$ROOT/tools/coverage.sh"'):
    raise SystemExit("dev coverage provisions the native SDK too late")

required_runner = (
    "docker.io/superkelvint/searchkernel-ci:latest",
    'IMAGE_ID=$(docker image inspect --format',
    'source_volume="searchkernel-dev-src-',
    'head_sha=$(git -C "$ROOT" rev-parse HEAD)',
    'target_volume=$(ci_runtime_target_volume "$ROOT" "$environment_id" "$head_sha")',
    '"io.searchkernel.source-head=$head_sha"',
    'git -C "$ROOT" bundle create "$bundle" HEAD',
    'commit_epoch=$(git -C /workspace show -s --format=%ct "$SOURCE_HEAD")',
    '(cd /workspace && git ls-files -z | xargs -0r touch -h -d "@$commit_epoch" --)',
    'find /workspace -path /workspace/.git -prune -o -type d -exec touch -h -d "@$commit_epoch" {} +',
    'git -C "$ROOT" diff --no-renames --name-only -z HEAD',
    'EXACT_SOURCE=${SEARCHKERNEL_CI_EXACT_SOURCE:-0}',
    'if [[ "$EXACT_SOURCE" == "1" ]]; then',
    'exact-source mode; using committed Git tree only',
    'git -C "$ROOT" ls-files --others --exclude-standard -z',
    '-v "$source_volume:/work"',
    '-v "$target_volume:/cargo-target"',
    "-e CARGO_TARGET_DIR=/cargo-target",
    'ci_runtime_require_digest "$vespa_ref" "$IMAGE"',
    'io.searchkernel.environment-id',
    'ci_runtime_create_volume "$target_volume" worktree-target cache',
    'ci_runtime_shared_download_volume cargo-registry "$environment_id"',
    'ci_runtime_create_volume "$cargo_registry_volume" immutable-download cache',
    '-v "$cargo_registry_volume:/root/.cargo/registry"',
    'ci_runtime_prune_target_volumes "$ROOT" "$target_volume"',
    'ci_runtime_prune_ephemeral_state',
    'ci_runtime_prune_legacy_ephemeral_state',
    'ci_runtime_prune_shared_download_volumes "$environment_id" "$shared_cargo_registry_volume"',
    'ci_runtime_create_volume "$source_volume" source "$ephemeral_lifecycle"',
    'ci_runtime_remove_invocation_containers "$invocation_id"',
    'lease_container="searchkernel-dev-lease-$invocation_id"',
    'lifecycle_lock=${SEARCHKERNEL_CI_LIFECYCLE_LOCK:-/tmp/searchkernel-ci-runtime-lifecycle.lock}',
    'flock 8',
    'flock -u 8',
    '-v "$target_volume:/lease/target:ro"',
    '-v "$native_volume:/lease/native-sdk:ro"',
    '-v "$sccache_socket_volume:/lease/sccache-socket:ro"',
    'ci_runtime_prune_sccache_generations "$sccache_binary_volume" "$sccache_socket_volume"',
    'ci_runtime_prune_vespa_volumes "$native_volume"',
    'io.searchkernel.invocation-id=$invocation_id',
    'io.searchkernel.worktree=$ROOT',
    'ensure-ci-sccache.sh',
    '-v "$sccache_binary_volume:/opt/searchkernel/sccache:ro"',
    '-v "$sccache_socket_volume:/sccache:ro"',
    "-e RUSTC_WRAPPER=/opt/searchkernel/sccache/sccache",
    "-e SCCACHE_SERVER_UDS=/sccache/sccache.sock",
    "-e SCCACHE_CLIENT_SIDE=1",
    "-e SCCACHE_IGNORE_SERVER_IO_ERROR=1",
    "-e FLATBUFFERS_CPP_ROOT=/usr/local",
    "-e FLATC_PATH=/usr/local/bin/flatc",
    "-e CC=/opt/rh/gcc-toolset-15/root/usr/bin/gcc",
    "-e CXX=/opt/rh/gcc-toolset-15/root/usr/bin/g++",
    "-e CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/opt/rh/gcc-toolset-15/root/usr/bin/gcc",
    "-e LIBRARY_PATH=/opt/rh/gcc-toolset-15/root/usr/lib64:/opt/rh/gcc-toolset-15/root/usr/lib",
    "-e SEARCHKERNEL_IN_CI_IMAGE=1",
    '[[ "${1:-}" == "coverage" ]] && COVERAGE_SNAPSHOT=1',
    'native-base:build',
    'native-base:verify',
    'test:native-base-artifact',
    '-e VESPA_ROOT=/native-base-producer/vespa',
    '-e VESPA_DEPS=/native-base-producer/vespa-deps',
    'container_args[3]="/native-base-output"',
    'container_args[2]="/native-base-input/$archive_name"',
    'artifact_volume="searchkernel-dev-artifacts-$invocation_id"',
    "-e SEARCHKERNEL_COVERAGE_DIR=/artifacts/coverage",
    'tar -C /artifacts -cf - coverage | tar -C "$ROOT" -xf -',
    'if [[ "$status" -eq 0 ]]; then',
    'coverage failed; no canonical report was exported',
)
for fragment in required_runner:
    if fragment not in runner:
        raise SystemExit(f"CI-image runner is missing {fragment!r}")

if 'SEARCHKERNEL_CI_EXACT_SOURCE: "1"' not in workflow:
    raise SystemExit("on-demand exact-SHA verification must disable working-tree overlays")

head_resolution = runner.index('head_sha=$(git -C "$ROOT" rev-parse HEAD)')
target_selection = runner.index('target_volume=$(ci_runtime_target_volume "$ROOT" "$environment_id" "$head_sha")')
if head_resolution > target_selection:
    raise SystemExit("persistent Cargo target selection must be keyed by the resolved source HEAD")

exact_guard = runner.index('if [[ "$EXACT_SOURCE" == "1" ]]; then')
overlay_command = runner.index('git -C "$ROOT" diff --no-renames --name-only -z HEAD')
snapshot_verify = runner.index("# Verify that the private Git repository still names the exact host HEAD")
if not (exact_guard < overlay_command < snapshot_verify):
    raise SystemExit("working-tree overlay is not guarded by exact-source mode")
if "else\n" not in runner[exact_guard:overlay_command]:
    raise SystemExit("exact-source mode does not bypass the working-tree overlay")

lock_acquire = runner.index('flock 8')
vespa_ensure = runner.index('ensure-ci-vespa-volume.sh')
sccache_ensure = runner.index('ensure-ci-sccache.sh')
ephemeral_prune = runner.index('ci_runtime_prune_ephemeral_state')
lease_create = runner.index('--name "$lease_container"')
if not (lock_acquire < ephemeral_prune < vespa_ensure < sccache_ensure < lease_create):
    raise SystemExit("lifecycle lock must cover stale cleanup and shared cache materialization before the invocation lease")
for prune in (
    'ci_runtime_prune_target_volumes "$ROOT" "$target_volume"',
    'ci_runtime_prune_shared_download_volumes "$environment_id" "$shared_cargo_registry_volume"',
    'ci_runtime_prune_sccache_generations "$sccache_binary_volume" "$sccache_socket_volume"',
    'ci_runtime_prune_vespa_volumes "$native_volume"',
):
    prune_index = runner.index(prune)
    if prune_index < lease_create:
        raise SystemExit(f"persistent cache pruning must happen after the invocation lease: {prune}")
    if prune_index > runner.index('flock -u 8'):
        raise SystemExit(f"persistent cache pruning must happen before lifecycle lock release: {prune}")

# /work is a Docker volume populated by a separate container. Its numeric
# ownership can vary across self-hosted runners, so both the exact-HEAD
# preflight and the real execution container must carry an explicit,
# process-local Git safe.directory policy.
for git_env in (
    "-e GIT_CONFIG_COUNT=1",
    "-e GIT_CONFIG_KEY_0=safe.directory",
    "-e GIT_CONFIG_VALUE_0=/work",
):
    if runner.count(git_env) < 2:
        raise SystemExit(
            f"CI-image runner must apply {git_env!r} to both Git-using containers"
        )

for forbidden in (
    "searchkernel-ci-cargo-git",
    "searchkernel-native-ci-lock",
    ":/root/.cargo/git",
    '-v "$ROOT:',
    "/work/.git/searchkernel/toolchains",
    "git config --global --add safe.directory /work",
):
    if forbidden in runner:
        raise SystemExit(f"CI-image runner reintroduced shared/host mutable state: {forbidden}")

print("dev CI-image source contract: PASS")
PY

# Persistent Cargo target state may be reused only for the same immutable
# source revision. Different HEADs must never collide, even for the same
# worktree and CI image environment.
source "$RUNTIME"
target_head_a=$(ci_runtime_target_volume "$ROOT" "test-environment" "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
target_head_a_repeat=$(ci_runtime_target_volume "$ROOT" "test-environment" "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
target_head_b=$(ci_runtime_target_volume "$ROOT" "test-environment" "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
[[ "$target_head_a" == "$target_head_a_repeat" ]] || {
    echo "Cargo target identity is not stable for the same source HEAD" >&2
    exit 1
}
[[ "$target_head_a" != "$target_head_b" ]] || {
    echo "Cargo target identity leaked across source HEAD revisions" >&2
    exit 1
}
if ci_runtime_target_volume "$ROOT" "test-environment" "main" >/dev/null 2>&1; then
    echo "Cargo target identity accepted a mutable/non-SHA source ref" >&2
    exit 1
fi
echo "dev CI-image HEAD-scoped Cargo target regression: PASS"

for fragment in \
    'ci_runtime_create_volume()' \
    'ci_runtime_prune_ephemeral_state()' \
    'ci_runtime_prune_legacy_ephemeral_state()' \
    'ci_runtime_prune_shared_download_volumes()' \
    'ci_runtime_prune_sccache_generations()' \
    'ci_runtime_prune_vespa_volumes()' \
    'io.searchkernel.owner=searchkernel' \
    'io.searchkernel.lifecycle'; do
    grep -F -- "$fragment" "$RUNTIME" >/dev/null || {
        echo "CI runtime ownership helper is missing lifecycle contract fragment: $fragment" >&2
        exit 1
    }
done

for fragment in \
    '"$CARGO" llvm-cov clean --workspace' \
    '--workspace' \
    '--all-targets' \
    '--all-features' \
    '--no-report' \
    '--summary-only' \
    'metadata.json' \
    '"working_tree_clean": not bool(git_status)' \
    '"native_cpp_coverage": False' \
    'refusing destructive coverage output directory'; do
    grep -F -- "$fragment" "$COVERAGE" >/dev/null || {
        echo "coverage runner is missing contract fragment: $fragment" >&2
        exit 1
    }
done
if grep -F -- 'cargo install' "$COVERAGE" >/dev/null; then
    echo 'coverage endpoint must not install/mutate the immutable CI toolchain' >&2
    exit 1
fi

tmp=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-dev-image-test.XXXXXX")
trap 'rm -rf -- "$tmp"' EXIT

repo="$tmp/repo"
clone="$tmp/clone"
bundle="$tmp/repository.bundle"
mkdir -p "$repo"
git -C "$repo" init -q
git -C "$repo" config user.name "CI Image Test"
git -C "$repo" config user.email "ci-image@example.invalid"
printf 'tracked\n' > "$repo/tracked.txt"
git -C "$repo" add tracked.txt
git -C "$repo" commit -q -m initial
head=$(git -C "$repo" rev-parse HEAD)

git -C "$repo" bundle create "$bundle" HEAD
git clone -q "$bundle" "$clone"
git -C "$clone" checkout -q --detach "$head"

[[ "$(git -C "$clone" rev-parse HEAD)" == "$head" ]] || {
    echo "git bundle snapshot did not preserve exact HEAD" >&2
    exit 1
}

echo "dev CI-image Git snapshot regression: PASS"
