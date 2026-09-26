#!/usr/bin/env bash
set -euo pipefail

# Maintain one runner-local sccache service per pinned sccache version. Build
# containers keep private source and CARGO_TARGET_DIR volumes and connect only
# to this non-authoritative compiler cache over a shared Unix socket.
#
# The pinned binary is also materialized into a versioned Docker volume. That
# makes rollout independent of whether searchkernel-ci:latest has already been
# rebuilt from the Dockerfile that now includes the same sccache version.
#
# Usage:
#   tools/ensure-ci-sccache.sh <ci-image>
#
# Prints "<binary-volume> <socket-volume>" on stdout. Diagnostics go to stderr.

[[ $# -eq 1 ]] || {
    echo "usage: $0 <ci-image>" >&2
    exit 2
}

CI_IMAGE=$1
SCCACHE_VERSION=0.17.0
SCCACHE_SHA256=67c4a96dd237c1f518f6b36083f270f9976d516f1e57fce891755ea782e50006

command -v docker >/dev/null 2>&1 || {
    echo "docker is required" >&2
    exit 1
}
command -v flock >/dev/null 2>&1 || {
    echo "flock is required" >&2
    exit 1
}

SAFE_VERSION=${SCCACHE_VERSION//./-}
TOOLCHAIN_ID=$(docker image inspect --format '{{index .Config.Labels "io.searchkernel.toolchain-id"}}' "$CI_IMAGE")
[[ "$TOOLCHAIN_ID" =~ ^[0-9a-f]{64}$ ]] || {
    echo "CI image is missing a valid io.searchkernel.toolchain-id label: ${TOOLCHAIN_ID:-<missing>}" >&2
    exit 1
}
CACHE_NAMESPACE=$TOOLCHAIN_ID
SERVER="searchkernel-ci-sccache-v$SAFE_VERSION-$CACHE_NAMESPACE"
BINARY_VOLUME="searchkernel-ci-sccache-bin-v$SAFE_VERSION"
CACHE_VOLUME="searchkernel-ci-sccache-cache-v$SAFE_VERSION-$CACHE_NAMESPACE"
SOCKET_VOLUME="searchkernel-ci-sccache-socket-v$SAFE_VERSION-$CACHE_NAMESPACE"
LOCK="${TMPDIR:-/tmp}/searchkernel-ci-sccache-v$SAFE_VERSION-$CACHE_NAMESPACE.lock"
SCCACHE_BIN=/opt/searchkernel/sccache/sccache

exec 9>"$LOCK"
flock 9

validate_binary() {
    docker volume inspect "$BINARY_VOLUME" >/dev/null 2>&1 || return 1
    docker run --rm \
        -v "$BINARY_VOLUME:/opt/searchkernel/sccache:ro" \
        "$CI_IMAGE" \
        "$SCCACHE_BIN" --version 2>/dev/null |
        grep -qx "sccache $SCCACHE_VERSION"
}

if ! validate_binary; then
    echo "Materializing pinned sccache v$SCCACHE_VERSION binary" >&2
    docker rm -f "$SERVER" >/dev/null 2>&1 || true
    docker volume rm -f "$BINARY_VOLUME" >/dev/null 2>&1 || true
    docker volume create "$BINARY_VOLUME" >/dev/null

    if docker run --rm "$CI_IMAGE" /usr/local/bin/sccache --version 2>/dev/null |
        grep -qx "sccache $SCCACHE_VERSION"; then
        echo "Seeding pinned sccache binary from the immutable CI image" >&2
        docker run --rm \
            -v "$BINARY_VOLUME:/out" \
            "$CI_IMAGE" \
            /bin/bash -lc 'install -m 0755 /usr/local/bin/sccache /out/sccache'
    else
        echo "CI image predates pinned sccache; downloading checksum-verified bootstrap binary" >&2
        docker run --rm \
            -e SCCACHE_VERSION="$SCCACHE_VERSION" \
            -e SCCACHE_SHA256="$SCCACHE_SHA256" \
            -v "$BINARY_VOLUME:/out" \
            "$CI_IMAGE" \
            /bin/bash -lc '
                set -euo pipefail
                work=$(mktemp -d /tmp/sccache.XXXXXX)
                trap "rm -rf -- \"$work\"" EXIT
                asset="sccache-v${SCCACHE_VERSION}-x86_64-unknown-linux-musl.tar.gz"
                curl --fail --location --retry 5 --silent --show-error \
                    "https://github.com/mozilla/sccache/releases/download/v${SCCACHE_VERSION}/$asset" \
                    -o "$work/$asset"
                printf "%s  %s\n" "$SCCACHE_SHA256" "$work/$asset" | sha256sum -c - >/dev/null
                tar -xzf "$work/$asset" -C "$work"
                install -m 0755 \
                    "$work/sccache-v${SCCACHE_VERSION}-x86_64-unknown-linux-musl/sccache" \
                    /out/sccache
            '
    fi

    validate_binary || {
        echo "materialized sccache binary failed validation" >&2
        docker volume rm -f "$BINARY_VOLUME" >/dev/null 2>&1 || true
        exit 1
    }
fi

validate_server() {
    [[ "$(docker inspect --format '{{.State.Running}}' "$SERVER" 2>/dev/null || true)" == "true" ]] || return 1
    docker run --rm \
        -e SCCACHE_SERVER_UDS=/sccache/sccache.sock \
        -v "$BINARY_VOLUME:/opt/searchkernel/sccache:ro" \
        -v "$SOCKET_VOLUME:/sccache:ro" \
        "$CI_IMAGE" \
        "$SCCACHE_BIN" --show-stats >/dev/null 2>&1
}

if ! validate_server; then
    echo "Starting shared sccache v$SCCACHE_VERSION server" >&2
    docker rm -f "$SERVER" >/dev/null 2>&1 || true
    docker volume create "$CACHE_VOLUME" >/dev/null
    docker volume create "$SOCKET_VOLUME" >/dev/null

    docker run -d \
        --name "$SERVER" \
        --restart unless-stopped \
        --label "io.searchkernel.sccache-version=$SCCACHE_VERSION" \
        --label "io.searchkernel.toolchain-id=$TOOLCHAIN_ID" \
        -e SCCACHE_DIR=/cache \
        -e SCCACHE_CACHE_SIZE=20G \
        -e SCCACHE_SERVER_UDS=/sccache/sccache.sock \
        -e SCCACHE_IDLE_TIMEOUT=0 \
        -e SCCACHE_START_SERVER=1 \
        -e SCCACHE_NO_DAEMON=1 \
        -v "$BINARY_VOLUME:/opt/searchkernel/sccache:ro" \
        -v "$CACHE_VOLUME:/cache" \
        -v "$SOCKET_VOLUME:/sccache" \
        "$CI_IMAGE" \
        /bin/bash -lc 'rm -f /sccache/sccache.sock; exec /opt/searchkernel/sccache/sccache' >/dev/null

    healthy=0
    for _ in $(seq 1 50); do
        if validate_server; then
            healthy=1
            break
        fi
        sleep 0.1
    done
    if [[ "$healthy" != "1" ]]; then
        echo "warning: shared sccache server failed to become healthy: $SERVER" >&2
        docker logs "$SERVER" >&2 || true
        docker rm -f "$SERVER" >/dev/null 2>&1 || true
        echo "warning: continuing with sccache's local-compiler fallback; the build remains authoritative" >&2
    fi
else
    echo "Reusing shared sccache v$SCCACHE_VERSION server" >&2
fi

printf '%s %s\n' "$BINARY_VOLUME" "$SOCKET_VOLUME"
