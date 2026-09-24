#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

candidate=${PYTHON:-}
if [[ -z "$candidate" && -x "$ROOT/venv/bin/python" ]]; then
    candidate="$ROOT/venv/bin/python"
fi
if [[ -z "$candidate" ]]; then
    candidate=$(command -v python3 2>/dev/null || true)
fi
if [[ -z "$candidate" ]]; then
    echo "error: no Python interpreter found; run ./dev doctor" >&2
    exit 1
fi
if [[ "$candidate" != */* ]]; then
    candidate=$(command -v "$candidate" 2>/dev/null || true)
fi
[[ -n "$candidate" && -x "$candidate" ]] || {
    echo "error: selected Python interpreter is not executable: ${PYTHON:-$candidate}" >&2
    exit 1
}

printf '%s\n' "$candidate"
