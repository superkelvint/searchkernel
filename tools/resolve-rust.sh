#!/usr/bin/env bash
set -euo pipefail

mode=${1:-cargo}
case "$mode" in
    cargo|rustc) ;;
    *)
        echo "usage: tools/resolve-rust.sh cargo|rustc" >&2
        exit 2
        ;;
esac

env_name=$(printf '%s' "$mode" | tr '[:lower:]' '[:upper:]')
candidate=${!env_name:-}
if [[ -z "$candidate" ]]; then
    candidate=$(command -v "$mode" 2>/dev/null || true)
fi
if [[ -z "$candidate" && -x "$HOME/.cargo/bin/$mode" ]]; then
    candidate="$HOME/.cargo/bin/$mode"
fi
if [[ -z "$candidate" ]]; then
    echo "error: could not resolve $mode; run ./dev doctor" >&2
    exit 1
fi
if [[ "$candidate" != */* ]]; then
    candidate=$(command -v "$candidate" 2>/dev/null || true)
fi
[[ -n "$candidate" && -x "$candidate" ]] || {
    echo "error: resolved $mode is not executable: ${candidate:-<empty>}" >&2
    exit 1
}

"$candidate" --version >/dev/null 2>&1 || {
    echo "error: resolved $mode is unusable: $candidate" >&2
    exit 1
}

printf '%s\n' "$candidate"
