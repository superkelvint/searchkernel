#!/usr/bin/env bash
set -euo pipefail

ROOT=${1:-$(git rev-parse --show-toplevel)}
cd "$ROOT"

status=0
checked=0

while IFS= read -r -d '' file; do
    [[ -f "$file" ]] || continue

    first_line=
    IFS= read -r first_line < "$file" || true

    interpreter=
    case "$first_line" in
        '#!'*bash*) interpreter=bash ;;
        '#!'*/sh|'#! '*'/sh'|'#! '*'/env sh'|'#! '*'/env -S sh'*) interpreter=sh ;;
        *) continue ;;
    esac

    checked=$((checked + 1))
    if ! "$interpreter" -n "$file"; then
        echo "shell syntax check failed: $file ($interpreter)" >&2
        status=1
    fi
done < <(git ls-files -z)

if ((checked == 0)); then
    echo "shell syntax check found no tracked shell scripts" >&2
    exit 1
fi

if ((status != 0)); then
    exit "$status"
fi

echo "shell syntax check PASS: $checked tracked scripts"
