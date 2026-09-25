#!/usr/bin/env bash
set -euo pipefail

mode=${1:-node}
case "$mode" in
    node|npm) ;;
    *)
        echo "usage: tools/resolve-node.sh node|npm" >&2
        exit 2
        ;;
esac

resolve_candidate() {
    local name=$1 env_name=$2 candidate
    candidate=${!env_name:-}
    if [[ -z "$candidate" ]]; then
        candidate=$(command -v "$name" 2>/dev/null || true)
    fi
    if [[ -n "$candidate" && "$candidate" != */* ]]; then
        candidate=$(command -v "$candidate" 2>/dev/null || true)
    fi
    [[ -n "$candidate" && -x "$candidate" ]] || return 1
    printf '%s\n' "$candidate"
}

node_bin=$(resolve_candidate node NODE) || {
    echo "error: could not resolve node; run ./dev doctor" >&2
    exit 1
}

version=$("$node_bin" -p 'process.versions.node' 2>/dev/null || true)
IFS=. read -r major minor patch <<< "$version"
if [[ ! "$major" =~ ^[0-9]+$ || ! "$minor" =~ ^[0-9]+$ ]] ||
   (( major < 20 || (major == 20 && minor < 19) )); then
    echo "error: Node.js >=20.19 is required; found ${version:-unknown} at $node_bin" >&2
    exit 1
fi

if [[ "$mode" == node ]]; then
    printf '%s\n' "$node_bin"
    exit 0
fi

npm_bin=$(resolve_candidate npm NPM) || {
    sibling="$(dirname -- "$node_bin")/npm"
    if [[ -x "$sibling" ]]; then
        npm_bin=$sibling
    else
        echo "error: could not resolve npm for $node_bin; run ./dev doctor" >&2
        exit 1
    fi
}
"$npm_bin" --version >/dev/null 2>&1 || {
    echo "error: resolved npm is unusable: $npm_bin" >&2
    exit 1
}
printf '%s\n' "$npm_bin"
