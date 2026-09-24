#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
SETUP="$ROOT/tools/actions-runner-setup.sh"

tmp="$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-actions-container-test.XXXXXX")"
trap 'rm -rf -- "$tmp"' EXIT

BLOCK="$tmp/container-block.sh"

awk '
    $0 == "# Persistent native build container" {
        in_block = 1
        next
    }
    in_block && $0 == "# Container setup + native build" {
        exit
    }
    in_block {
        print
    }
' "$SETUP" >"$BLOCK"

grep -Fq 'EXISTING_WORK_MOUNT=' "$BLOCK" || {
    echo "failed to extract persistent container lifecycle block" >&2
    exit 1
}

run_block() {
    local exists=$1
    local work_mount=$2
    local running=$3
    local log=$4

    : >"$log"

    CONTAINER_NAME=searchkernel-native
    REPO_ROOT=/repo
    HOST_ARTIFACT_CACHE=/artifacts
    CARGO_VOLUME=searchkernel-cargo
    RUSTUP_VOLUME=searchkernel-rustup
    NATIVE_CACHE_VOLUME=searchkernel-native-cache
    BUILD_ENV=ci
    CONTAINER_CARGO_TARGET_DIR=/work/target/actions-runner-alma
    IMAGE=example.invalid/searchkernel-native:test

    section() {
        :
    }

    docker() {
        local command=${1:-}
        shift || true

        case "$command" in
            container)
                [[ "${1:-}" == "inspect" ]] || return 90
                [[ "$exists" == "1" ]]
                ;;
            inspect)
                [[ "${1:-}" == "--format" ]] || return 91
                local template=${2:-}
                case "$template" in
                    *'.Destination "/work"'*) printf '%s\n' "$work_mount" ;;
                    *'.State.Running'*) printf '%s\n' "$running" ;;
                    *) echo "unexpected docker inspect template: $template" >&2; return 92 ;;
                esac
                ;;
            start)
                printf 'start %s\n' "$*" >>"$log"
                ;;
            run)
                printf 'run %s\n' "$*" >>"$log"
                ;;
            *)
                echo "unexpected docker command: $command $*" >&2
                return 93
                ;;
        esac
    }

    source "$BLOCK"
}

absent_log="$tmp/absent.log"
run_block 0 "" false "$absent_log"
grep -Fq "run -d --name searchkernel-native" "$absent_log"
grep -Fq -- "--privileged" "$absent_log"
grep -Fq -- "--pids-limit=-1" "$absent_log"
grep -Fq -- "-v /repo:/work" "$absent_log"
grep -Fq -- "-v /artifacts:/native-artifacts:ro" "$absent_log"
grep -Fq -- "-e CARGO_TARGET_DIR=/work/target/actions-runner-alma" "$absent_log"

stopped_log="$tmp/stopped.log"
run_block 1 /repo false "$stopped_log"
grep -Fq "start searchkernel-native" "$stopped_log"
if grep -Fq "run " "$stopped_log"; then
    echo "matching stopped container must be restarted, not recreated" >&2
    exit 1
fi

mismatch_log="$tmp/mismatch.log"
mismatch_err="$tmp/mismatch.err"
if (run_block 1 /other-checkout true "$mismatch_log") 2>"$mismatch_err"; then
    echo "container mounted to another checkout must fail closed" >&2
    exit 1
fi
grep -Fq "belongs to another checkout" "$mismatch_err"
if [[ -s "$mismatch_log" ]]; then
    echo "checkout mismatch must not start or recreate the existing container" >&2
    cat "$mismatch_log" >&2
    exit 1
fi

echo "PASS actions runner container lifecycle ownership and restart/create behavior"
