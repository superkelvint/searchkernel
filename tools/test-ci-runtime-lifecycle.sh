#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
RUNTIME="$ROOT/tools/ci-runtime-ownership.sh"

bash -n "$RUNTIME"
# shellcheck source=ci-runtime-ownership.sh
source "$RUNTIME"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-ci-runtime-test.XXXXXX")
trap 'rm -rf -- "$tmp"' EXIT
log="$tmp/docker.log"
: > "$log"

docker() {
    printf '%s\n' "$*" >> "$log"

    if [[ "${1:-}" == "ps" && "${2:-}" == "-aq" ]]; then
        case "$*" in
            *"label=io.searchkernel.lifecycle=ephemeral"*)
                printf '%s\n' stale-container
                ;;
            *"volume=live-volume"*)
                printf '%s\n' live-holder
                ;;
            *)
                ;;
        esac
        return 0
    fi

    if [[ "${1:-}" == "inspect" ]]; then
        local name=${*: -1}
        case "$*" in
            *'io.searchkernel.created-at'*)
                [[ "$name" == "stale-container" ]] && printf '%s\n' 10000
                ;;
            *'io.searchkernel.toolchain-id'*)
                case "$name" in
                    old-server) printf '%s\n' old-toolchain ;;
                    current-server) printf '%s\n' current-toolchain ;;
                esac
                ;;
            *'{{.Name}}'*)
                printf '/%s\n' "$name"
                ;;
            *'{{.Created}}'*)
                printf '%s\n' '1970-01-01T00:00:00Z'
                ;;
        esac
        return 0
    fi

    if [[ "${1:-}" == "volume" && "${2:-}" == "ls" ]]; then
        case "$*" in
            *"label=io.searchkernel.lifecycle=ephemeral"*)
                printf '%s\n' stale-volume live-volume fresh-volume
                ;;
            *"label=io.searchkernel.cache-kind=immutable-download"*)
                printf '%s\n' current-download old-download
                ;;
            *)
                printf '%s\n' \
                    searchkernel-ci-sccache-cache-v0-17-0-old-toolchain \
                    searchkernel-ci-sccache-socket-v0-17-0-old-toolchain \
                    searchkernel-ci-sccache-cache-v0-17-0-current-toolchain \
                    searchkernel-ci-sccache-socket-v0-17-0-current-toolchain \
                    searchkernel-ci-sccache-cache-v0-17-0 \
                    searchkernel-ci-sccache-socket-v0-17-0 \
                    searchkernel-ci-vespa-old \
                    searchkernel-ci-vespa-current
                ;;
        esac
        return 0
    fi

    if [[ "${1:-}" == "volume" && "${2:-}" == "inspect" ]]; then
        local name=${*: -1}
        case "$*" in
            *'io.searchkernel.created-at'*)
                case "$name" in
                    stale-volume|live-volume) printf '%s\n' 10000 ;;
                    fresh-volume) printf '%s\n' 19500 ;;
                esac
                ;;
            *'io.searchkernel.environment-id'*)
                case "$name" in
                    current-download) printf '%s\n' env-current ;;
                    old-download) printf '%s\n' env-old ;;
                esac
                ;;
            *'io.searchkernel.owner'*)
                printf '%s\n' ''
                ;;
            *'{{.CreatedAt}}'*)
                printf '%s\n' '1970-01-01T00:00:00Z'
                ;;
        esac
        return 0
    fi

    return 0
}

export SEARCHKERNEL_CI_NOW_EPOCH=20000
export SEARCHKERNEL_CI_CACHE_RETIRE_TTL_SECONDS=0

ci_runtime_create_volume generated-volume source ephemeral \
    io.searchkernel.environment-id=env-current \
    io.searchkernel.invocation-id=test-run >/dev/null

for fragment in \
    'volume create' \
    '--label io.searchkernel.owner=searchkernel' \
    '--label io.searchkernel.cache-kind=source' \
    '--label io.searchkernel.lifecycle=ephemeral' \
    '--label io.searchkernel.created-at=20000' \
    '--label io.searchkernel.environment-id=env-current' \
    '--label io.searchkernel.invocation-id=test-run' \
    'generated-volume'; do
    grep -F -- "$fragment" "$log" >/dev/null || {
        echo "missing lifecycle creation fragment: $fragment" >&2
        exit 1
    }
done

: > "$log"
ci_runtime_prune_ephemeral_state 3600
grep -F 'rm -f stale-container' "$log" >/dev/null
grep -F 'volume rm -f stale-volume' "$log" >/dev/null
if grep -F 'volume rm -f live-volume' "$log" >/dev/null; then
    echo "live ephemeral volume was pruned" >&2
    exit 1
fi
if grep -F 'volume rm -f fresh-volume' "$log" >/dev/null; then
    echo "fresh ephemeral volume was pruned" >&2
    exit 1
fi

: > "$log"
ci_runtime_prune_shared_download_volumes env-current current-download
grep -F 'volume rm -f old-download' "$log" >/dev/null
if grep -F 'volume rm -f current-download' "$log" >/dev/null; then
    echo "current download cache was pruned" >&2
    exit 1
fi

: > "$log"
# Exercise both a toolchain rotation and a future sccache-version rotation.
docker() {
    printf '%s\n' "$*" >> "$log"
    if [[ "${1:-}" == "ps" && "${2:-}" == "-aq" ]]; then
        case "$*" in
            *"label=io.searchkernel.sccache-version"*)
                printf '%s\n' old-toolchain-server old-version-server current-server
                ;;
            *)
                ;;
        esac
        return 0
    fi
    if [[ "${1:-}" == "inspect" ]]; then
        local name=${*: -1}
        case "$*" in
            *'.Destination "/sccache"'*)
                case "$name" in
                    old-toolchain-server) printf '%s\n' 'searchkernel-ci-sccache-socket-v0-17-0-old-toolchain' ;;
                    old-version-server) printf '%s\n' 'searchkernel-ci-sccache-socket-v0-16-0-old-version-toolchain' ;;
                    current-server) printf '%s\n' 'searchkernel-ci-sccache-socket-v0-17-0-current-toolchain' ;;
                esac
                ;;
        esac
        return 0
    fi
    if [[ "${1:-}" == "volume" && "${2:-}" == "ls" ]]; then
        printf '%s\n' \
            searchkernel-ci-sccache-bin-v0-16-0 \
            searchkernel-ci-sccache-cache-v0-16-0-old-version-toolchain \
            searchkernel-ci-sccache-socket-v0-16-0-old-version-toolchain \
            searchkernel-ci-sccache-cache-v0-17-0-old-toolchain \
            searchkernel-ci-sccache-socket-v0-17-0-old-toolchain \
            searchkernel-ci-sccache-bin-v0-17-0 \
            searchkernel-ci-sccache-cache-v0-17-0-current-toolchain \
            searchkernel-ci-sccache-socket-v0-17-0-current-toolchain \
            searchkernel-ci-vespa-old \
            searchkernel-ci-vespa-current
        return 0
    fi
    return 0
}

ci_runtime_prune_sccache_generations \
    searchkernel-ci-sccache-bin-v0-17-0 \
    searchkernel-ci-sccache-socket-v0-17-0-current-toolchain
grep -F 'rm -f old-toolchain-server' "$log" >/dev/null
grep -F 'rm -f old-version-server' "$log" >/dev/null
grep -F 'volume rm -f searchkernel-ci-sccache-cache-v0-17-0-old-toolchain' "$log" >/dev/null
grep -F 'volume rm -f searchkernel-ci-sccache-socket-v0-17-0-old-toolchain' "$log" >/dev/null
grep -F 'volume rm -f searchkernel-ci-sccache-bin-v0-16-0' "$log" >/dev/null
grep -F 'volume rm -f searchkernel-ci-sccache-cache-v0-16-0-old-version-toolchain' "$log" >/dev/null
grep -F 'volume rm -f searchkernel-ci-sccache-socket-v0-16-0-old-version-toolchain' "$log" >/dev/null
if grep -F 'rm -f current-server' "$log" >/dev/null; then
    echo "current sccache server was pruned" >&2
    exit 1
fi
if grep -F 'volume rm -f searchkernel-ci-sccache-bin-v0-17-0' "$log" >/dev/null; then
    echo "current sccache binary was pruned" >&2
    exit 1
fi
if grep -F 'volume rm -f searchkernel-ci-sccache-cache-v0-17-0-current-toolchain' "$log" >/dev/null; then
    echo "current sccache generation was pruned" >&2
    exit 1
fi

: > "$log"
ci_runtime_prune_vespa_volumes searchkernel-ci-vespa-current
grep -F 'volume rm -f searchkernel-ci-vespa-old' "$log" >/dev/null
if grep -F 'volume rm -f searchkernel-ci-vespa-current' "$log" >/dev/null; then
    echo "current Vespa generation was pruned" >&2
    exit 1
fi

echo "Search Kernel Docker lifecycle regression: PASS"
