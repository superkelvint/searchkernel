#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

if [ "${SK_ALLOW_DIRTY_HARNESS:-0}" = "1" ]; then
    exit 0
fi

if git -C "$ROOT/../.." rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    STATUS=$(git -C "$ROOT/../.." status --porcelain -- reference/vespa || true)
    if [ -n "$STATUS" ]; then
        echo "reference/vespa is dirty; verifier harness must be immutable:" >&2
        printf '%s\n' "$STATUS" >&2
        exit 1
    fi
fi
