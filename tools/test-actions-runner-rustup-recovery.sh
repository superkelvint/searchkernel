#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
SETUP="$ROOT/tools/actions-runner-setup.sh"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-actions-rustup-test.XXXXXX")
trap 'rm -rf -- "$tmp"' EXIT

RUST_BLOCK="$tmp/rust-block.sh"

awk '
    $0 == "# Rust" {
        in_rust = 1
        next
    }
    in_rust && $0 == "# FlatBuffers" {
        exit
    }
    in_rust {
        print
    }
' "$SETUP" >"$RUST_BLOCK"

grep -q 'say "Checking Rust"' "$RUST_BLOCK" ||
    {
        echo "failed to extract Rust provisioning block" >&2
        exit 1
    }

run_case() {
    local initial_state=$1
    local expected_installs=$2
    local expected_uninstalls=$3
    local uninstall_fails=${4:-0}

    local state=$initial_state
    local installs=0
    local uninstalls=0

    say() {
        :
    }

    die() {
        echo "ERROR: $*" >&2
        return 1
    }

    curl() {
        echo "unexpected rustup bootstrap download" >&2
        return 91
    }

    rustup() {
        local command=${1:-}
        shift || true

        case "$command" in
            toolchain)
                local action=${1:-}
                shift || true
                case "$action" in
                    list)
                        if [[ "$state" != "missing" ]]; then
                            echo "stable-x86_64-unknown-linux-gnu (active, default)"
                        fi
                        ;;
                    uninstall)
                        [[ "${1:-}" == "stable" ]] || return 92
                        uninstalls=$((uninstalls + 1))
                        if [[ "$uninstall_fails" == "1" ]]; then
                            return 1
                        fi
                        state="missing"
                        ;;
                    install)
                        [[ "$*" == "stable --profile minimal --force" ]] || return 93
                        installs=$((installs + 1))
                        state="healthy"
                        ;;
                    *)
                        echo "unexpected rustup toolchain action: $action" >&2
                        return 94
                        ;;
                esac
                ;;
            run)
                [[ "${1:-}" == "stable" ]] || return 95
                local tool=${2:-}
                local arg=${3:-}
                if [[ "$state" != "healthy" ]]; then
                    echo "error: missing manifest in toolchain 'stable-x86_64-unknown-linux-gnu'" >&2
                    return 1
                fi
                case "$tool:$arg" in
                    rustc:--version)
                        echo "rustc test"
                        ;;
                    cargo:--version)
                        echo "cargo test"
                        ;;
                    *)
                        echo "unexpected rustup run invocation: $*" >&2
                        return 96
                        ;;
                esac
                ;;
            default)
                [[ "${1:-}" == "stable" ]] || return 97
                if [[ "$state" != "healthy" ]]; then
                    echo "error: missing manifest in toolchain 'stable-x86_64-unknown-linux-gnu'" >&2
                    return 1
                fi
                ;;
            *)
                echo "unexpected rustup command: $command $*" >&2
                return 98
                ;;
        esac
    }

    rustc() {
        [[ "${1:-}" == "--version" ]] || return 99
        [[ "$state" == "healthy" ]] || return 1
        echo "rustc test"
    }

    cargo() {
        [[ "${1:-}" == "--version" ]] || return 100
        [[ "$state" == "healthy" ]] || return 1
        echo "cargo test"
    }

    source "$RUST_BLOCK"

    [[ "$state" == "healthy" ]] ||
        {
            echo "Rust provisioning did not leave a usable toolchain" >&2
            return 1
        }

    [[ "$installs" -eq "$expected_installs" ]] ||
        {
            echo "expected $expected_installs install(s), got $installs" >&2
            return 1
        }

    [[ "$uninstalls" -eq "$expected_uninstalls" ]] ||
        {
            echo "expected $expected_uninstalls uninstall(s), got $uninstalls" >&2
            return 1
        }
}

run_case healthy 0 0
run_case corrupt 1 1
run_case corrupt 1 1 1

echo "PASS actions runner reuses healthy Rust and repairs corrupt persisted stable toolchains even when uninstall fails"
