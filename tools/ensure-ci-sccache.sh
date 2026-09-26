#!/usr/bin/env bash
set -euo pipefail

# Maintain one runner-local sccache server per pinned sccache version. Build
# containers keep private source and CARGO_TARGET_DIR volumes and connect only
# to this non-authoritative compiler cache over a shared Unix socket.
#
# Usage:
#   tools/ensure-ci-sccache.sh <ci-image>
#
# Prints only the Docker socket-volume name on stdout. Diagnostics go to stderr.

[[ $# -eq 1 ]] || {
    echo "usage: $0 <ci-image>" >&2
    exit 2
}

CI_IMAGE=$1

command -v docker >/dev/null 2>&1 || {
    echo "docker is required" >&2
    exit 1
}
command -v flock >/dev/null 2>&1 || {
    echo "flock is required" >&2
    exit 1
}

VERSION=$(docker run --rm "$CI_IMAGE" sccache --version | awk 'NR == 1 { print $2 }')
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
    echo "could not resolve pinned sccache version from $CI_IMAGE: $VERSION" >&2
    exit 1
}

SAFE_VERSION=${VERSION//./-}
SERVER="searchkernel-ci-sccache-v$SAFE_VERSION"
CACHE_VOLUME="searchkernel-ci-sccache-cache-v$SAFE_VERSION"
SOCKET_VOLUME="searchkernel-ci-sccache-socket-v$SAFE_VERSION"
LOCK="${TMPDIR:-/tmp}/searchkernel-ci-sccache-v$SAFE_VERSION.lock"

exec 9>"$LOCK"
flock 9

validate_server() {
    [[ "$(docker inspect --format '{{.State.Running}}' "$SERVER" 2>/dev/null || true)" == "true" ]] || return 1
    docker run --rm \
        -e SCCACHE_SERVER_UDS=/sccache/sccache.sock \
        -v "$SOCKET_VOLUME:/sccache:ro" \
        "$CI_IMAGE" \
        sccache --show-stats >/dev/null 2>&1
}

if ! validate_server; then
    echo "Starting shared sccache v$VERSION server" >&2
    docker rm -f "$SERVER" >/dev/null 2>&1 || true
    docker volume create "$CACHE_VOLUME" >/dev/null
    docker volume create "$SOCKET_VOLUME" >/dev/null

    docker run -d \
        --name "$SERVER" \
        --restart unless-stopped \
        --label "io.searchkernel.sccache-version=$VERSION" \
        -e SCCACHE_DIR=/cache \
        -e SCCACHE_CACHE_SIZE=20G \
        -e SCCACHE_SERVER_UDS=/sccache/sccache.sock \
        -e SCCACHE_IDLE_TIMEOUT=0 \
        -e SCCACHE_NO_DAEMON=1 \
        -v "$CACHE_VOLUME:/cache" \
        -v "$SOCKET_VOLUME:/sccache" \
        "$CI_IMAGE" \
        /bin/bash -lc 'rm -f /sccache/sccache.sock; exec sccache --start-server' >/dev/null

    healthy=0
    for _ in $(seq 1 50); do
        if validate_server; then
            healthy=1
            break
        fi
        sleep 0.1
    done
    if [[ "$healthy" != "1" ]]; then
        echo "shared sccache server failed to become healthy: $SERVER" >&2
        docker logs "$SERVER" >&2 || true
        docker rm -f "$SERVER" >/dev/null 2>&1 || true
        exit 1
    fi
else
    echo "Reusing shared sccache v$VERSION server" >&2
fi

printf '%s\n' "$SOCKET_VOLUME"
