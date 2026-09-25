#!/usr/bin/env bash
set -euo pipefail

# Materialize one immutable Vespa image digest into a Docker volume shared
# read-only by CI jobs. The volume is content-addressed by the image digest and
# is never a build/output cache.
#
# Usage:
#   tools/ensure-ci-vespa-volume.sh <ci-image> <vespa-image@sha256:...> <vespa-commit> <bundle-id>
#
# Prints only the Docker volume name on stdout; diagnostics go to stderr.

[[ $# -eq 4 ]] || {
    echo "usage: $0 <ci-image> <vespa-image@sha256:...> <vespa-commit> <bundle-id>" >&2
    exit 2
}

CI_IMAGE=$1
VESPA_IMAGE=$2
VESPA_COMMIT=$3
VESPA_BUNDLE_ID=$4

command -v docker >/dev/null 2>&1 || {
    echo "docker is required" >&2
    exit 1
}
command -v flock >/dev/null 2>&1 || {
    echo "flock is required" >&2
    exit 1
}

case "$VESPA_IMAGE" in
    *@sha256:*) ;;
    *)
        echo "Vespa image must be an immutable digest reference: $VESPA_IMAGE" >&2
        exit 1
        ;;
esac

DIGEST=${VESPA_IMAGE##*@sha256:}
[[ "$DIGEST" =~ ^[0-9a-f]{64}$ ]] || {
    echo "invalid Vespa image digest: $DIGEST" >&2
    exit 1
}

VOLUME="searchkernel-ci-vespa-${DIGEST:0:32}"
LOCK="${TMPDIR:-/tmp}/searchkernel-ci-vespa-$DIGEST.lock"
SEED_CONTAINER="searchkernel-ci-vespa-seed-${DIGEST:0:20}-$$"

exec 9>"$LOCK"
flock 9

validate_volume() {
    docker volume inspect "$VOLUME" >/dev/null 2>&1 || return 1
    docker run --rm \
        -e EXPECTED_COMMIT="$VESPA_COMMIT" \
        -e EXPECTED_BUNDLE="$VESPA_BUNDLE_ID" \
        -e EXPECTED_IMAGE="$VESPA_IMAGE" \
        -v "$VOLUME:/opt/searchkernel/native-sdk-cache:ro" \
        "$CI_IMAGE" \
        /bin/bash -lc '
            set -euo pipefail
            cache=/opt/searchkernel/native-sdk-cache/ci/x86_64-unknown-linux-gnu
            base="$cache/objects/$EXPECTED_COMMIT"
            toolchain="$cache/toolchains/$EXPECTED_COMMIT"
            test -s "$base/lib/libvespalib.so"
            test -f "$toolchain/upstream/vespa/.searchkernel-vespa-artifact.json"
            test "$(cat /opt/searchkernel/native-sdk-cache/.searchkernel-vespa-image-digest)" = "$EXPECTED_IMAGE"
            "$PYTHON" - "$base/manifest.json" "$toolchain/upstream/vespa/.searchkernel-vespa-artifact.json" <<'"'"'PY'"'"'
import json
import os
import sys

base_path, marker_path = sys.argv[1:]
with open(base_path, encoding="utf-8") as stream:
    base = json.load(stream)
with open(marker_path, encoding="utf-8") as stream:
    marker = json.load(stream)
if base.get("vespa_commit") != os.environ["EXPECTED_COMMIT"]:
    raise SystemExit("mounted Vespa base commit mismatch")
if marker.get("vespa_commit") != os.environ["EXPECTED_COMMIT"]:
    raise SystemExit("mounted Vespa toolchain commit mismatch")
PY
        ' >/dev/null
}

if ! validate_volume; then
    echo "Materializing immutable Vespa cache volume $VOLUME" >&2
    docker rm -f "$SEED_CONTAINER" >/dev/null 2>&1 || true
    docker volume rm -f "$VOLUME" >/dev/null 2>&1 || true
    docker volume create "$VOLUME" >/dev/null

    # The Vespa image is scratch-based, so it has no executable we can run.
    # Export its filesystem locally and stream only the native-cache subtree
    # into the named volume using the toolchain-bearing CI image. This is a
    # one-time local materialization per Vespa digest, not a registry push.
    docker create --name "$SEED_CONTAINER" "$VESPA_IMAGE" /bin/true >/dev/null
    docker export "$SEED_CONTAINER" |
        docker run --rm -i \
            -v "$VOLUME:/opt/searchkernel/native-sdk-cache" \
            "$CI_IMAGE" \
            /bin/bash -lc '
                set -euo pipefail
                tar -x -C /opt/searchkernel/native-sdk-cache \
                    --strip-components=3 \
                    opt/searchkernel/native-sdk-cache
            '
    docker rm "$SEED_CONTAINER" >/dev/null

    # Add only the resolver alias and an integrity marker. Jobs mount this
    # volume read-only, so it is immutable after this locked initialization.
    docker run --rm \
        -e VESPA_COMMIT="$VESPA_COMMIT" \
        -e VESPA_IMAGE="$VESPA_IMAGE" \
        -v "$VOLUME:/opt/searchkernel/native-sdk-cache" \
        "$CI_IMAGE" \
        /bin/bash -lc '
            set -euo pipefail
            cache=/opt/searchkernel/native-sdk-cache/ci/x86_64-unknown-linux-gnu
            mkdir -p "$cache/checkouts"
            ln -sfn "$cache/toolchains/$VESPA_COMMIT/upstream/vespa" "$cache/checkouts/$VESPA_COMMIT"
            printf "%s\n" "$VESPA_IMAGE" > /opt/searchkernel/native-sdk-cache/.searchkernel-vespa-image-digest
        '

    validate_volume || {
        echo "materialized Vespa volume failed validation: $VOLUME" >&2
        docker volume rm -f "$VOLUME" >/dev/null 2>&1 || true
        exit 1
    }
else
    echo "Reusing immutable Vespa cache volume $VOLUME" >&2
fi

printf '%s\n' "$VOLUME"
