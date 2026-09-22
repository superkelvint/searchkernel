#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
BASE_REF=${1:-}

if [[ -z "$BASE_REF" ]]; then
    echo "usage: $0 <base-commit>" >&2
    exit 2
fi

cd "$ROOT"

if ! git cat-file -e "$BASE_REF^{commit}" 2>/dev/null; then
    echo "base commit $BASE_REF is not available; checkout must fetch it" >&2
    exit 1
fi

base_dirs=$(
    git ls-tree -d --name-only "$BASE_REF:reference/persistence/fixtures" 2>/dev/null         | grep '^manifest-v[0-9][0-9]*-storage-v[0-9][0-9]*$' || true
)

if [[ -z "$base_dirs" ]]; then
    echo "Persistence fixture immutability PASS (no historical fixture directories on base)"
    exit 0
fi

changed=$(
    git diff --name-only "$BASE_REF"...HEAD -- reference/persistence/fixtures || true
)

while IFS= read -r dir; do
    [[ -n "$dir" ]] || continue
    if grep -q "^reference/persistence/fixtures/$dir/" <<<"$changed"; then
        echo "historical persistence fixture is immutable and may not be modified: $dir" >&2
        git diff --name-status "$BASE_REF"...HEAD -- "reference/persistence/fixtures/$dir" >&2 || true
        exit 1
    fi
done <<<"$base_dirs"

echo "Persistence fixture immutability PASS"
