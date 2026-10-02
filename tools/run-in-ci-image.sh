#!/usr/bin/env bash
set -euo pipefail

CONTROL_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
ROOT=${SEARCHKERNEL_CI_SOURCE_ROOT:-$CONTROL_ROOT}
ROOT=$(cd -- "$ROOT" && pwd)
source "$CONTROL_ROOT/tools/ci-runtime-ownership.sh"
IMAGE=${SEARCHKERNEL_CI_IMAGE:-docker.io/superkelvint/searchkernel-ci:latest}
PULL_MODE=${SEARCHKERNEL_CI_PULL:-always}
KEEP_STATE=${SEARCHKERNEL_CI_KEEP_STATE:-0}
EXACT_SOURCE=${SEARCHKERNEL_CI_EXACT_SOURCE:-0}
COVERAGE_SNAPSHOT=0
[[ "${1:-}" == "coverage" ]] && COVERAGE_SNAPSHOT=1
CONSUMER_RELEASE_EXPORT=${SEARCHKERNEL_CONSUMER_RELEASE_EXPORT:-}
NATIVE_BASE_NEEDS_PRODUCER=0
NATIVE_BASE_OUTPUT=
NATIVE_BASE_VERIFY_ARCHIVE=
VESPA_PRODUCER_INPUT=
DEPS_PRODUCER_INPUT=
container_args=("$@")

die() {
    echo "error: $*" >&2
    exit 1
}

case "${1:-}:${2:-}" in
    native-base:build)
        [[ "$#" -ge 4 && "$#" -le 5 ]] ||
            die "usage: ./dev native-base build <host|ci> <output-dir> [generation]"
        NATIVE_BASE_NEEDS_PRODUCER=1
        NATIVE_BASE_OUTPUT=$4
        mkdir -p "$NATIVE_BASE_OUTPUT"
        NATIVE_BASE_OUTPUT=$(cd -- "$NATIVE_BASE_OUTPUT" && pwd -P)
        if find "$NATIVE_BASE_OUTPUT" -mindepth 1 -print -quit | grep -q .; then
            die "native-base output directory must be empty: $NATIVE_BASE_OUTPUT"
        fi
        ;;
    native-base:verify)
        [[ "$#" -eq 6 ]] ||
            die "usage: ./dev native-base verify <archive> <host|ci> <target> <vespa-commit>"
        NATIVE_BASE_VERIFY_ARCHIVE=$3
        [[ -f "$NATIVE_BASE_VERIFY_ARCHIVE" && -f "$NATIVE_BASE_VERIFY_ARCHIVE.sha256" ]] ||
            die "native-base verification requires an archive and checksum sidecar: $NATIVE_BASE_VERIFY_ARCHIVE"
        NATIVE_BASE_VERIFY_ARCHIVE=$(cd -- "$(dirname -- "$NATIVE_BASE_VERIFY_ARCHIVE")" && pwd -P)/$(basename -- "$NATIVE_BASE_VERIFY_ARCHIVE")
        ;;
    test:native-base-artifact)
        NATIVE_BASE_NEEDS_PRODUCER=1
        ;;
    test:consumer-release)
        if [[ -n "$CONSUMER_RELEASE_EXPORT" ]]; then
            mkdir -p "$CONSUMER_RELEASE_EXPORT"
            CONSUMER_RELEASE_EXPORT=$(cd -- "$CONSUMER_RELEASE_EXPORT" && pwd -P)
            if find "$CONSUMER_RELEASE_EXPORT" -mindepth 1 -print -quit | grep -q .; then
                die "consumer-release export directory must be empty: $CONSUMER_RELEASE_EXPORT"
            fi
        fi
        ;;
esac

if [[ -n "$CONSUMER_RELEASE_EXPORT" &&
      "${1:-}:${2:-}" != "test:consumer-release" ]]; then
    die "SEARCHKERNEL_CONSUMER_RELEASE_EXPORT is only valid with ./dev test consumer-release"
fi

if [[ "$NATIVE_BASE_NEEDS_PRODUCER" == "1" ]]; then
    VESPA_PRODUCER_INPUT=${VESPA_ROOT:-}
    DEPS_PRODUCER_INPUT=${VESPA_DEPS:-}
    [[ -n "$VESPA_PRODUCER_INPUT" && -d "$VESPA_PRODUCER_INPUT" ]] ||
        die "native-base container execution requires VESPA_ROOT to the configured pinned producer checkout"
    [[ -n "$DEPS_PRODUCER_INPUT" && -d "$DEPS_PRODUCER_INPUT/include" ]] ||
        die "native-base container execution requires VESPA_DEPS to the producer dependency installation"
    VESPA_PRODUCER_INPUT=$(cd -- "$VESPA_PRODUCER_INPUT" && pwd -P)
    DEPS_PRODUCER_INPUT=$(cd -- "$DEPS_PRODUCER_INPUT" && pwd -P)
fi

case "$EXACT_SOURCE" in
    0|1) ;;
    *) die "SEARCHKERNEL_CI_EXACT_SOURCE must be 0 or 1" ;;
esac

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
vespa_bundle=$(docker image inspect --format '{{index .Config.Labels "io.searchkernel.vespa-bundle-id"}}' "$IMAGE_ID")
vespa_ref=$(docker image inspect --format '{{index .Config.Labels "io.searchkernel.vespa-image-digest"}}' "$IMAGE_ID")
vespa_ref=$(ci_runtime_require_digest "$vespa_ref" "$IMAGE") || exit $?
environment_id=$(docker image inspect --format '{{index .Config.Labels "io.searchkernel.environment-id"}}' "$IMAGE_ID")
[[ -n "$environment_id" && "$environment_id" != "<no value>" ]] ||
    die "$IMAGE is missing the io.searchkernel.environment-id label"

case "$PULL_MODE" in
    always)
        echo "==> dev: pulling external Vespa input $vespa_ref"
        docker pull "$vespa_ref" >/dev/null
        ;;
    missing)
        if ! docker image inspect "$vespa_ref" >/dev/null 2>&1; then
            echo "==> dev: pulling missing external Vespa input $vespa_ref"
            docker pull "$vespa_ref" >/dev/null
        fi
        ;;
    never)
        docker image inspect "$vespa_ref" >/dev/null 2>&1 ||
            die "$vespa_ref is not available locally and SEARCHKERNEL_CI_PULL=never"
        ;;
esac
[[ "$vespa_ref" == *@sha256:* ]] || die "could not resolve immutable Vespa image digest"

# Serialize shared Docker-state selection/materialization/retirement. Image pulls
# happen before this point so slow registry I/O does not serialize independent
# runners. The Vespa and sccache ensure helpers mutate shared named state and
# therefore share the same lock as stale cleanup and later cache retirement.
lifecycle_lock=${SEARCHKERNEL_CI_LIFECYCLE_LOCK:-/tmp/searchkernel-ci-runtime-lifecycle.lock}
exec 8>"$lifecycle_lock"
flock 8

ci_runtime_prune_ephemeral_state
ci_runtime_prune_legacy_ephemeral_state

native_volume=$(bash "$CONTROL_ROOT/tools/ensure-ci-vespa-volume.sh" "$IMAGE_ID" "$vespa_ref" "$label" "$vespa_bundle")
read -r sccache_binary_volume sccache_socket_volume < <(bash "$CONTROL_ROOT/tools/ensure-ci-sccache.sh" "$IMAGE_ID")

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
invocation_id="$(id -u)-$$-${RANDOM}"
resource_created_at=$(ci_runtime_now_epoch)
ephemeral_lifecycle=ephemeral
[[ "$KEEP_STATE" == "1" ]] && ephemeral_lifecycle=retained

source_volume="searchkernel-dev-src-$invocation_id"
shared_cargo_registry_volume=$(ci_runtime_shared_download_volume cargo-registry "$environment_id")
ephemeral_target=${SEARCHKERNEL_CI_EPHEMERAL_TARGET:-0}
if [[ "$ephemeral_target" == "1" ]]; then
    target_volume="searchkernel-ci-target-$invocation_id"
    # Remote exact-SHA verification executes untrusted repository code. Keep
    # Cargo's writable registry/source state job-private alongside CARGO_TARGET_DIR
    # so one subject revision cannot poison a later verification job.
    cargo_registry_volume="searchkernel-ci-cargo-registry-$invocation_id"
else
    target_volume=$(ci_runtime_target_volume "$ROOT" "$environment_id")
    cargo_registry_volume=$shared_cargo_registry_volume
fi

artifact_volume=""
if [[ "$COVERAGE_SNAPSHOT" == "1" || -n "$CONSUMER_RELEASE_EXPORT" ]]; then
    artifact_volume="searchkernel-dev-artifacts-$invocation_id"
fi
producer_vespa_volume=""
producer_deps_volume=""
native_base_output_volume=""
native_base_input_volume=""
if [[ "$NATIVE_BASE_NEEDS_PRODUCER" == "1" ]]; then
    producer_vespa_volume="searchkernel-native-base-vespa-$invocation_id"
    producer_deps_volume="searchkernel-native-base-deps-$invocation_id"
fi
if [[ -n "$NATIVE_BASE_OUTPUT" ]]; then
    native_base_output_volume="searchkernel-native-base-output-$invocation_id"
fi
if [[ -n "$NATIVE_BASE_VERIFY_ARCHIVE" ]]; then
    native_base_input_volume="searchkernel-native-base-input-$invocation_id"
fi
container_name="searchkernel-dev-$invocation_id"
lease_container="searchkernel-dev-lease-$invocation_id"
docker_resource_labels=(
    --label io.searchkernel.owner=searchkernel
    --label "io.searchkernel.lifecycle=$ephemeral_lifecycle"
    --label "io.searchkernel.created-at=$resource_created_at"
    --label "io.searchkernel.invocation-id=$invocation_id"
)

cleanup() {
    local status=$?
    rm -rf -- "$tmp"
    # The lease exists only to protect selected persistent volumes during this
    # invocation. Remove all invocation-owned containers even when debug volumes
    # are intentionally retained.
    ci_runtime_remove_invocation_containers "$invocation_id"
    if [[ "$KEEP_STATE" == "1" ]]; then
        echo "Search Kernel CI-image volumes retained:"
        echo "  source volume: $source_volume"
        echo "  persistent target volume: $target_volume"
        echo "  immutable/download Cargo registry volume: $cargo_registry_volume"
        [[ -n "$artifact_volume" ]] && echo "  artifact volume: $artifact_volume"
        [[ -n "$producer_vespa_volume" ]] && echo "  native-base Vespa producer volume: $producer_vespa_volume"
        [[ -n "$producer_deps_volume" ]] && echo "  native-base dependency producer volume: $producer_deps_volume"
        [[ -n "$native_base_output_volume" ]] && echo "  native-base output volume: $native_base_output_volume"
        [[ -n "$native_base_input_volume" ]] && echo "  native-base input volume: $native_base_input_volume"
        return "$status"
    fi

    docker volume rm -f "$source_volume" >/dev/null 2>&1 || true
    if [[ "$ephemeral_target" == "1" ]]; then
        docker volume rm -f "$target_volume" >/dev/null 2>&1 || true
        docker volume rm -f "$cargo_registry_volume" >/dev/null 2>&1 || true
    fi
    [[ -z "$artifact_volume" ]] || docker volume rm -f "$artifact_volume" >/dev/null 2>&1 || true
    [[ -z "$producer_vespa_volume" ]] || docker volume rm -f "$producer_vespa_volume" >/dev/null 2>&1 || true
    [[ -z "$producer_deps_volume" ]] || docker volume rm -f "$producer_deps_volume" >/dev/null 2>&1 || true
    [[ -z "$native_base_output_volume" ]] || docker volume rm -f "$native_base_output_volume" >/dev/null 2>&1 || true
    [[ -z "$native_base_input_volume" ]] || docker volume rm -f "$native_base_input_volume" >/dev/null 2>&1 || true
    return "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# The lifecycle lock was acquired before shared cache ensure/materialization.
# Keep it through invocation leases and retirement, then release before source
# snapshotting/compilation so ordinary ./dev invocations still run concurrently.
ci_runtime_create_volume "$source_volume" source "$ephemeral_lifecycle" \
    "io.searchkernel.environment-id=$environment_id" \
    "io.searchkernel.invocation-id=$invocation_id" >/dev/null
if [[ "$ephemeral_target" == "1" ]]; then
    ci_runtime_create_volume "$cargo_registry_volume" job-cargo-registry "$ephemeral_lifecycle" \
        "io.searchkernel.environment-id=$environment_id" \
        "io.searchkernel.invocation-id=$invocation_id" >/dev/null
    ci_runtime_create_volume "$target_volume" job-target "$ephemeral_lifecycle" \
        "io.searchkernel.environment-id=$environment_id" \
        "io.searchkernel.invocation-id=$invocation_id" >/dev/null
else
    ci_runtime_create_volume "$cargo_registry_volume" immutable-download cache \
        "io.searchkernel.environment-id=$environment_id" >/dev/null
    ci_runtime_create_volume "$target_volume" worktree-target cache \
        "io.searchkernel.environment-id=$environment_id" \
        "io.searchkernel.worktree=$ROOT" >/dev/null
fi
if [[ -n "$artifact_volume" ]]; then
    ci_runtime_create_volume "$artifact_volume" artifact "$ephemeral_lifecycle" \
        "io.searchkernel.environment-id=$environment_id" \
        "io.searchkernel.invocation-id=$invocation_id" >/dev/null
fi
for special_volume in "$producer_vespa_volume" "$producer_deps_volume" "$native_base_output_volume" "$native_base_input_volume"; do
    [[ -z "$special_volume" ]] && continue
    ci_runtime_create_volume "$special_volume" native-base-io "$ephemeral_lifecycle" \
        "io.searchkernel.environment-id=$environment_id" \
        "io.searchkernel.invocation-id=$invocation_id" >/dev/null
done

# Pin every selected persistent generation before retiring older generations.
# A stopped container still owns its volume references, closing the race between
# "selected" and the later build-container mount.
lease_volume_args=(
    -v "$target_volume:/lease/target:ro"
    -v "$cargo_registry_volume:/lease/cargo-registry:ro"
    -v "$native_volume:/lease/native-sdk:ro"
    -v "$sccache_binary_volume:/lease/sccache-bin:ro"
    -v "$sccache_socket_volume:/lease/sccache-socket:ro"
)
[[ -z "$producer_vespa_volume" ]] || lease_volume_args+=(-v "$producer_vespa_volume:/lease/native-base-vespa:ro")
[[ -z "$producer_deps_volume" ]] || lease_volume_args+=(-v "$producer_deps_volume:/lease/native-base-deps:ro")
[[ -z "$native_base_output_volume" ]] || lease_volume_args+=(-v "$native_base_output_volume:/lease/native-base-output:ro")
[[ -z "$native_base_input_volume" ]] || lease_volume_args+=(-v "$native_base_input_volume:/lease/native-base-input:ro")
docker create \
    --name "$lease_container" \
    "${docker_resource_labels[@]}" \
    "${lease_volume_args[@]}" \
    "$IMAGE_ID" /bin/true >/dev/null

if [[ "$ephemeral_target" != "1" ]]; then
    ci_runtime_prune_target_volumes "$ROOT" "$target_volume"
    ci_runtime_prune_shared_download_volumes "$environment_id" "$shared_cargo_registry_volume"
fi
ci_runtime_prune_sccache_generations "$sccache_binary_volume" "$sccache_socket_volume"
ci_runtime_prune_vespa_volumes "$native_volume"
flock -u 8

snapshot_native_base_input() {
    local source=$1 volume=$2 label=$3
    echo "==> dev: snapshotting native-base $label input"
    tar -C "$source" -cf - . |
        docker run --rm -i "${docker_resource_labels[@]}" \
            -v "$volume:/snapshot" "$IMAGE_ID" tar -C /snapshot -xf -
}

if [[ "$NATIVE_BASE_NEEDS_PRODUCER" == "1" ]]; then
    snapshot_native_base_input "$VESPA_PRODUCER_INPUT" "$producer_vespa_volume" "Vespa producer"
    snapshot_native_base_input "$DEPS_PRODUCER_INPUT" "$producer_deps_volume" "dependency producer"
fi
if [[ -n "$NATIVE_BASE_VERIFY_ARCHIVE" ]]; then
    archive_parent=$(dirname -- "$NATIVE_BASE_VERIFY_ARCHIVE")
    archive_name=$(basename -- "$NATIVE_BASE_VERIFY_ARCHIVE")
    tar -C "$archive_parent" -cf - "$archive_name" "$archive_name.sha256" |
        docker run --rm -i "${docker_resource_labels[@]}" \
            -v "$native_base_input_volume:/native-base-input" "$IMAGE_ID" \
            tar -C /native-base-input -xf -
    container_args[2]="/native-base-input/$archive_name"
fi
if [[ -n "$NATIVE_BASE_OUTPUT" ]]; then
    container_args[3]="/native-base-output"
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
    docker run --rm -i         "${docker_resource_labels[@]}"         -e SOURCE_HEAD="$head_sha"         -v "$source_volume:/workspace"         "$IMAGE_ID"         /bin/bash -lc '
            set -euo pipefail
            bundle=/tmp/searchkernel.bundle
            cat > "$bundle"
            git clone --quiet "$bundle" /workspace
            git -C /workspace checkout --quiet --detach "$SOURCE_HEAD"
            # Cargo fingerprints include source mtimes. A fresh checkout would
            # otherwise make unchanged tracked sources newer than the persistent
            # target tree and force workspace recompilation on every container.
            commit_epoch=$(git -C /workspace show -s --format=%ct "$SOURCE_HEAD")
            (cd /workspace && git ls-files -z | xargs -0r touch -h -d "@$commit_epoch" --)
            find /workspace -path /workspace/.git -prune -o -type d -exec touch -h -d "@$commit_epoch" {} +
            rm -f "$bundle"
        '

if [[ "$EXACT_SOURCE" == "1" ]]; then
    echo "==> dev: exact-source mode; using committed Git tree only"
else
# Overlay exactly the non-ignored working-tree files that differ from HEAD.
# This preserves local edits/new source files while deliberately excluding
# target/, node_modules/, venvs, local SDKs, and all other ignored build state.
git -C "$ROOT" diff --no-renames --name-only -z HEAD --diff-filter=ACMRTUXB > "$overlay"
git -C "$ROOT" ls-files --others --exclude-standard -z >> "$overlay"

if [[ -s "$overlay" ]]; then
    tar -C "$ROOT" --null --files-from="$overlay" -cf - |
        docker run --rm -i             "${docker_resource_labels[@]}"             -v "$source_volume:/work"             "$IMAGE_ID"             tar -C /work -xf -
fi

git -C "$ROOT" diff --no-renames --name-only -z HEAD --diff-filter=D > "$deleted"
if [[ -s "$deleted" ]]; then
    cat "$deleted" |
        docker run --rm -i             "${docker_resource_labels[@]}"             -v "$source_volume:/work"             "$IMAGE_ID"             python3 -c '
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

fi

# Verify that the private Git repository still names the exact host HEAD before
# executing any build/test command.
actual=$(docker run --rm \
    "${docker_resource_labels[@]}" \
    -e GIT_CONFIG_COUNT=1 \
    -e GIT_CONFIG_KEY_0=safe.directory \
    -e GIT_CONFIG_VALUE_0=/work \
    -v "$source_volume:/work" \
    -w /work \
    "$IMAGE_ID" \
    git rev-parse HEAD)
[[ "${actual,,}" == "${head_sha,,}" ]] ||
    die "private source snapshot expected HEAD $head_sha, found $actual"

docker_env=(
    -e SEARCHKERNEL_IN_CI_IMAGE=1
    -e SEARCHKERNEL_BUILD_ENV=ci
    -e CARGO_TARGET_DIR=/cargo-target
    -e RUSTC_WRAPPER=/opt/searchkernel/sccache/sccache
    -e SCCACHE_SERVER_UDS=/sccache/sccache.sock
    -e SCCACHE_CLIENT_SIDE=1
    -e SCCACHE_IGNORE_SERVER_IO_ERROR=1
    -e FLATBUFFERS_CPP_ROOT=/usr/local
    -e FLATC_PATH=/usr/local/bin/flatc
    -e CC=/opt/rh/gcc-toolset-15/root/usr/bin/gcc
    -e CXX=/opt/rh/gcc-toolset-15/root/usr/bin/g++
    -e AR=/opt/rh/gcc-toolset-15/root/usr/bin/ar
    -e RANLIB=/opt/rh/gcc-toolset-15/root/usr/bin/ranlib
    -e LIBRARY_PATH=/opt/rh/gcc-toolset-15/root/usr/lib64:/opt/rh/gcc-toolset-15/root/usr/lib
    -e CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/opt/rh/gcc-toolset-15/root/usr/bin/gcc
    # The source snapshot is created in a separate container and then mounted
    # into this one. Docker volume ownership can differ across self-hosted
    # runners, so make the repository trust policy explicit and process-local
    # instead of relying on runner state or a mutable global Git config.
    -e GIT_CONFIG_COUNT=1
    -e GIT_CONFIG_KEY_0=safe.directory
    -e GIT_CONFIG_VALUE_0=/work
)

# Preserve Search Kernel test knobs and diagnostics, but never forward host
# native/toolchain overrides such as SK_NATIVE_SDK, VESPA_ROOT, or
# CARGO_TARGET_DIR into the canonical image. Native-base production is the
# deliberate exception: declared producer trees are snapshotted into private
# invocation volumes above and exposed at fixed container paths.
while IFS= read -r name; do
    case "$name" in
        SEARCHKERNEL_MODEL_*|SEARCHKERNEL_VERIFY_*|RUST_BACKTRACE|RUST_LOG|CARGO_TERM_COLOR|CARGO_NET_OFFLINE)
            docker_env+=(--env "$name")
            ;;
    esac
done < <(compgen -e)

if [[ "$NATIVE_BASE_NEEDS_PRODUCER" == "1" ]]; then
    docker_env+=(
        -e VESPA_ROOT=/native-base-producer/vespa
        -e VESPA_DEPS=/native-base-producer/vespa-deps
    )
fi

docker_volume_args=(
    -v "$source_volume:/work"
    -v "$target_volume:/cargo-target"
    -v "$cargo_registry_volume:/root/.cargo/registry"
    -v "$native_volume:/opt/searchkernel/native-sdk-cache:ro"
    -v "$sccache_binary_volume:/opt/searchkernel/sccache:ro"
    -v "$sccache_socket_volume:/sccache:ro"
)
if [[ -n "$artifact_volume" ]]; then
    docker_volume_args+=(
        -v "$artifact_volume:/artifacts"
    )
fi
if [[ "$COVERAGE_SNAPSHOT" == "1" ]]; then
    docker_env+=(
        -e SEARCHKERNEL_COVERAGE_DIR=/artifacts/coverage
    )
fi
if [[ -n "$CONSUMER_RELEASE_EXPORT" ]]; then
    docker_env+=(
        -e SEARCHKERNEL_CONSUMER_RELEASE_OUTPUT=/artifacts/consumer-release
    )
fi
[[ -z "$producer_vespa_volume" ]] || docker_volume_args+=(-v "$producer_vespa_volume:/native-base-producer/vespa:ro")
[[ -z "$producer_deps_volume" ]] || docker_volume_args+=(-v "$producer_deps_volume:/native-base-producer/vespa-deps:ro")
[[ -z "$native_base_output_volume" ]] || docker_volume_args+=(-v "$native_base_output_volume:/native-base-output")
[[ -z "$native_base_input_volume" ]] || docker_volume_args+=(-v "$native_base_input_volume:/native-base-input:ro")

tty_args=(-i)
if [[ -t 0 && -t 1 ]]; then
    tty_args+=(-t)
fi

echo "==> dev: running in $IMAGE ($IMAGE_ID)"
status=0
docker run --rm     --name "$container_name"     "${docker_resource_labels[@]}"     --pids-limit=-1     "${tty_args[@]}"     "${docker_volume_args[@]}"     -w /work     "${docker_env[@]}"     "$IMAGE_ID"     ./dev "${container_args[@]}" || status=$?

if [[ -n "$NATIVE_BASE_OUTPUT" ]]; then
    if [[ "$status" -eq 0 ]]; then
        docker run --rm "${docker_resource_labels[@]}" \
            -v "$native_base_output_volume:/native-base-output:ro" "$IMAGE_ID" \
            tar -C /native-base-output -cf - . | tar -C "$NATIVE_BASE_OUTPUT" -xf -
        if ! find "$NATIVE_BASE_OUTPUT" -mindepth 1 -print -quit | grep -q .; then
            die "native-base build succeeded without producing exported artifacts"
        fi
        echo "==> dev: exported native-base artifacts to $NATIVE_BASE_OUTPUT"
    else
        echo "==> dev: native-base build failed; no artifacts were exported" >&2
    fi
fi

if [[ -n "$CONSUMER_RELEASE_EXPORT" ]]; then
    if [[ "$status" -eq 0 ]]; then
        if ! docker run --rm "${docker_resource_labels[@]}" -v "$artifact_volume:/artifacts:ro" "$IMAGE_ID"             test -f /artifacts/consumer-release/manifest.json; then
            die "consumer-release qualification succeeded without producing a release manifest"
        fi
        docker run --rm "${docker_resource_labels[@]}"             -v "$artifact_volume:/artifacts:ro" "$IMAGE_ID"             tar -C /artifacts/consumer-release -cf - . |
            tar -C "$CONSUMER_RELEASE_EXPORT" -xf -
        echo "==> dev: exported qualified consumer release to $CONSUMER_RELEASE_EXPORT"
    else
        echo "==> dev: consumer-release qualification failed; no artifacts were exported" >&2
    fi
fi

if [[ "$COVERAGE_SNAPSHOT" == "1" ]]; then
    rm -rf -- "$ROOT/coverage"
    if [[ "$status" -eq 0 ]]; then
        if ! docker run --rm "${docker_resource_labels[@]}" -v "$artifact_volume:/artifacts" "$IMAGE_ID" test -d /artifacts/coverage; then
            die "coverage command succeeded without producing /artifacts/coverage"
        fi
        docker run --rm "${docker_resource_labels[@]}" -v "$artifact_volume:/artifacts" "$IMAGE_ID" \
            tar -C /artifacts -cf - coverage | tar -C "$ROOT" -xf -
        echo "==> dev: exported coverage reports to $ROOT/coverage"
    else
        echo "==> dev: coverage failed; no canonical report was exported" >&2
    fi
fi

exit "$status"
