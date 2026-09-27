#!/usr/bin/env bash
set -euo pipefail

mode=${1:-}
case "$mode" in
    php)
        name=php
        env_name=PHP
        ;;
    ruby)
        name=ruby
        env_name=RUBY
        ;;
    *)
        echo "usage: tools/resolve-runtime.sh php|ruby" >&2
        exit 2
        ;;
esac

candidate=${!env_name:-}
if [[ -z "$candidate" ]]; then
    candidate=$(command -v "$name" 2>/dev/null || true)
elif [[ "$candidate" != */* ]]; then
    candidate=$(command -v "$candidate" 2>/dev/null || true)
fi

[[ -n "$candidate" && -x "$candidate" ]] || {
    echo "error: could not resolve $name; run ./dev doctor or use the canonical CI image" >&2
    exit 1
}

"$candidate" --version >/dev/null 2>&1 || {
    echo "error: resolved $name is unusable: $candidate" >&2
    exit 1
}

printf '%s\n' "$candidate"
