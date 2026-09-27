#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

[[ "$#" -ge 1 ]] || {
    echo "usage: tools/run-verifier.sh <semantic-target> [verifier-args...]" >&2
    exit 2
}

exec "$ROOT/reference/vespa/verify-through.sh" "$@"
