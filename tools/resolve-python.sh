#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

usable_python() {
    local candidate=$1
    [[ -x "$candidate" ]] || return 1
    "$candidate" - <<'PY' >/dev/null 2>&1
import sys
raise SystemExit(0 if sys.version_info >= (3, 10) else 1)
PY
}

resolve_named() {
    local candidate=$1
    if [[ "$candidate" != */* ]]; then
        candidate=$(command -v "$candidate" 2>/dev/null || true)
    fi
    [[ -n "$candidate" ]] || return 1
    printf '%s\n' "$candidate"
}

explicit=${PYTHON:-${CI_PYTHON:-}}
if [[ -n "$explicit" ]]; then
    candidate=$(resolve_named "$explicit" || true)
    if [[ -n "$candidate" ]] && usable_python "$candidate"; then
        printf '%s\n' "$candidate"
        exit 0
    fi
    echo "error: selected Python must be executable Python >=3.10: $explicit" >&2
    exit 1
fi

if [[ -x "$ROOT/venv/bin/python" ]] && usable_python "$ROOT/venv/bin/python"; then
    printf '%s\n' "$ROOT/venv/bin/python"
    exit 0
fi

candidate=$(command -v python3 2>/dev/null || true)
if [[ -n "$candidate" ]] && usable_python "$candidate"; then
    printf '%s\n' "$candidate"
    exit 0
fi

echo "error: no usable Python >=3.10 found; run ./dev doctor or provision Python first" >&2
exit 1
