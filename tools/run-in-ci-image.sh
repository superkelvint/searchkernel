#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
IMAGE=${SEARCHKERNEL_CI_IMAGE:-docker.io/superkelvint/searchkernel-ci:latest}
PULL_MODE=${SEARCHKERNEL_CI_PULL:-always}
KEEP_STATE=${SEARCHKERNEL_CI_KEEP_STATE:-0}
PRECOMMIT_SNAPSHOT=0
COVERAGE_SNAPSHOT=0
[[ "${1:-}" == "precommit-verify" ]] && PRECOMMIT_SNAPSHOT=1
[[ "${1:-}" == "coverage" ]] && COVERAGE_SNAPSHOT=1

die() {
    echo "error: $*" >&2
    exit 1
}

command -v docker >/dev/null 2>&1 ||
    die "Docker is required for Search Kernel build/test/verify commands. Set SEARCHKERNEL_DEV_USE_CI_IMAGE=0 for an intentional host run."

git -C "$ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1 ||
    die "Search Kernel CI-image runner requires a Git worktree"

case "$PULL_MODE" in
    always)
        echo "==> dev: pulling $IMAGE"
        docker pull "$IMAGE" >/dev/null
        ;;
    missing)
        if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
            echo "==> dev: pulling missing $IMAGE"
            docker pull "$IMAGE" >/dev/null
        fi
        ;;
    never)
        docker image inspect "$IMAGE" >/dev/null 2>&1 ||
            die "$IMAGE is not available locally and SEARCHKERNEL_CI_PULL=never"
        ;;
    *)
        die "SEARCHKERNEL_CI_PULL must be one of: always, missing, never"
        ;;
esac

IMAGE_ID=$(docker image inspect --format '{{.Id}}' "$IMAGE")
[[ -n "$IMAGE_ID" ]] || die "could not resolve immutable image ID for $IMAGE"

label=$(docker image inspect --format '{{index .Config.Labels "io.searchkernel.vespa-commit"}}' "$IMAGE_ID")
[[ -n "$label" && "$label" != "<no value>" ]] ||
    die "$IMAGE is missing the io.searchkernel.vespa-commit label"

docker run --rm "$IMAGE_ID" test -f /opt/searchkernel/native-validation-pass ||
    die "$IMAGE is missing its native validation marker"

docker run --rm "$IMAGE_ID" /bin/bash -lc '
    set -euo pipefail
    test -x /opt/rh/gcc-toolset-15/root/usr/bin/gcc
    test -x /opt/rh/gcc-toolset-15/root/usr/bin/g++
    test "$(/opt/rh/gcc-toolset-15/root/usr/bin/gcc -dumpfullversion -dumpversion | cut -d. -f1)" -ge 15
    test "$(/opt/rh/gcc-toolset-15/root/usr/bin/g++ -dumpfullversion -dumpversion | cut -d. -f1)" -ge 15
' || die "$IMAGE does not contain the required GCC 15 toolchain"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-ci-image.XXXXXX")
source_volume="searchkernel-dev-src-$(id -u)-$$-${RANDOM}"
target_volume="searchkernel-dev-target-$(id -u)-$$-${RANDOM}"
artifact_volume=""
if [[ "$COVERAGE_SNAPSHOT" == "1" ]]; then
    artifact_volume="searchkernel-dev-artifacts-$(id -u)-$$-${RANDOM}"
fi
container_name="searchkernel-dev-$(id -u)-$$-${RANDOM}"

cleanup() {
    local status=$?
    rm -rf -- "$tmp"
    if [[ "$KEEP_STATE" == "1" ]]; then
        echo "Search Kernel CI-image volumes retained:"
        echo "  source volume: $source_volume"
        echo "  target volume: $target_volume"
        [[ -n "$artifact_volume" ]] && echo "  artifact volume: $artifact_volume"
        return "$status"
    fi

    docker rm -f "$container_name" >/dev/null 2>&1 || true
    docker volume rm -f "$source_volume" "$target_volume" >/dev/null 2>&1 || true
    [[ -z "$artifact_volume" ]] || docker volume rm -f "$artifact_volume" >/dev/null 2>&1 || true
    return "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

docker volume create "$source_volume" >/dev/null
docker volume create "$target_volume" >/dev/null
if [[ -n "$artifact_volume" ]]; then
    docker volume create "$artifact_volume" >/dev/null
fi

head_sha=$(git -C "$ROOT" rev-parse HEAD)
bundle="$tmp/repository.bundle"
overlay="$tmp/overlay.paths"
deleted="$tmp/deleted.paths"

# A bundle avoids Docker-outside-of-Docker bind-mount ambiguity and works for
# ordinary clones and linked Git worktrees. The container receives private Git
# metadata/history instead of a .git pointer back into the host checkout.
git -C "$ROOT" bundle create "$bundle" HEAD

echo "==> dev: creating private source snapshot at $head_sha"
cat "$bundle" |
    docker run --rm -i         -e SOURCE_HEAD="$head_sha"         -v "$source_volume:/workspace"         "$IMAGE_ID"         /bin/bash -lc '
            set -euo pipefail
            bundle=/tmp/searchkernel.bundle
            cat > "$bundle"
            git clone --quiet "$bundle" /workspace
            git -C /workspace checkout --quiet --detach "$SOURCE_HEAD"
            git -C /workspace config core.hooksPath .githooks
            rm -f "$bundle"
        '

# Overlay exactly the non-ignored working-tree files that differ from HEAD.
# This preserves local edits/new source files while deliberately excluding
# target/, node_modules/, venvs, local SDKs, and all other ignored build state.
git -C "$ROOT" diff --no-renames --name-only -z HEAD --diff-filter=ACMRTUXB > "$overlay"
if [[ "$PRECOMMIT_SNAPSHOT" != "1" ]]; then
    # Interactive/local dev commands include non-ignored untracked source so a
    # developer can test a new file before committing it. Pre-commit instead
    # verifies exactly the staged snapshot; relevant untracked files were
    # already rejected by the host phase.
    git -C "$ROOT" ls-files --others --exclude-standard -z >> "$overlay"
fi

if [[ -s "$overlay" ]]; then
    tar -C "$ROOT" --null --files-from="$overlay" -cf - |
        docker run --rm -i             -v "$source_volume:/work"             "$IMAGE_ID"             tar -C /work -xf -
fi

git -C "$ROOT" diff --no-renames --name-only -z HEAD --diff-filter=D > "$deleted"
if [[ -s "$deleted" ]]; then
    cat "$deleted" |
        docker run --rm -i             -v "$source_volume:/work"             "$IMAGE_ID"             python3 -c '
import os
import shutil
import sys

root = os.path.realpath("/work")
for raw in sys.stdin.buffer.read().split(b"\0"):
    if not raw:
        continue
    rel = os.fsdecode(raw)
    target = os.path.realpath(os.path.join(root, rel))
    if target != root and not target.startswith(root + os.sep):
        raise SystemExit(f"refusing unsafe deletion path: {rel!r}")
    if os.path.isdir(target) and not os.path.islink(target):
        shutil.rmtree(target)
    else:
        try:
            os.unlink(target)
        except FileNotFoundError:
            pass
'
fi

if [[ "$PRECOMMIT_SNAPSHOT" == "1" ]]; then
    # The host hook rejects tracked unstaged changes before dispatch. Staging
    # the private overlay therefore recreates the exact commit snapshot and
    # preserves git-diff/index semantics for verifier scripts.
    docker run --rm \
        -v "$source_volume:/work" \
        -w /work \
        "$IMAGE_ID" \
        /bin/bash -lc 'git config --global --add safe.directory /work && git add -A && git diff --quiet'
fi

# Verify that the private Git repository still names the exact host HEAD before
# executing any build/test command.
actual=$(docker run --rm     -v "$source_volume:/work"     -w /work     "$IMAGE_ID"     /bin/bash -lc 'git config --global --add safe.directory /work && git rev-parse HEAD')
[[ "${actual,,}" == "${head_sha,,}" ]] ||
    die "private source snapshot expected HEAD $head_sha, found $actual"

docker_env=(
    -e SEARCHKERNEL_IN_CI_IMAGE=1
    -e SEARCHKERNEL_BUILD_ENV=ci
    -e CARGO_TARGET_DIR=/cargo-target
    -e FLATBUFFERS_CPP_ROOT=/usr/local
    -e FLATC_PATH=/usr/local/bin/flatc
    -e CC=/opt/rh/gcc-toolset-15/root/usr/bin/gcc
    -e CXX=/opt/rh/gcc-toolset-15/root/usr/bin/g++
    -e AR=/opt/rh/gcc-toolset-15/root/usr/bin/ar
    -e RANLIB=/opt/rh/gcc-toolset-15/root/usr/bin/ranlib
    -e LIBRARY_PATH=/opt/rh/gcc-toolset-15/root/usr/lib64:/opt/rh/gcc-toolset-15/root/usr/lib
    -e CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/opt/rh/gcc-toolset-15/root/usr/bin/gcc
)

# Preserve Search Kernel test knobs and diagnostics, but never forward host
# native/toolchain overrides such as SK_NATIVE_SDK, VESPA_ROOT, or
# CARGO_TARGET_DIR into the canonical image.
while IFS= read -r name; do
    case "$name" in
        SEARCHKERNEL_PRECOMMIT_*|SEARCHKERNEL_MODEL_*|SEARCHKERNEL_VERIFY_*|RUST_BACKTRACE|RUST_LOG|CARGO_TERM_COLOR|CARGO_NET_OFFLINE)
            docker_env+=(--env "$name")
            ;;
    esac
done < <(compgen -e)

docker_volume_args=(
    -v "$source_volume:/work"
    -v "$target_volume:/cargo-target"
)
if [[ -n "$artifact_volume" ]]; then
    docker_volume_args+=(
        -v "$artifact_volume:/artifacts"
    )
    docker_env+=(
        -e SEARCHKERNEL_COVERAGE_DIR=/artifacts/coverage
    )
fi

tty_args=(-i)
if [[ -t 0 && -t 1 ]]; then
    tty_args+=(-t)
fi

echo "==> dev: running in $IMAGE ($IMAGE_ID)"
status=0
docker run --rm     --name "$container_name"     --pids-limit=-1     "${tty_args[@]}"     "${docker_volume_args[@]}"     -w /work     "${docker_env[@]}"     "$IMAGE_ID"     ./dev "$@" || status=$?

if [[ "$COVERAGE_SNAPSHOT" == "1" ]]; then
    rm -rf -- "$ROOT/coverage"
    if [[ "$status" -eq 0 ]]; then
        if ! docker run --rm -v "$artifact_volume:/artifacts" "$IMAGE_ID" test -d /artifacts/coverage; then
            die "coverage command succeeded without producing /artifacts/coverage"
        fi
        docker run --rm -v "$artifact_volume:/artifacts" "$IMAGE_ID" \
            tar -C /artifacts -cf - coverage | tar -C "$ROOT" -xf -
        echo "==> dev: exported coverage reports to $ROOT/coverage"
    else
        echo "==> dev: coverage failed; no canonical report was exported" >&2
    fi
fi

exit "$status"
