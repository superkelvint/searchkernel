#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

"$ROOT/common/check-hashes.sh"
"$ROOT/common/check-harness-clean.sh"

case "${1:-}" in
    m0)
        "$ROOT/m0/verify-m0.sh"
        ;;
    m1)
        "$ROOT/m0/verify-m0.sh"
        "$ROOT/m1/verify-m1.sh"
        ;;
    m2)
        "$ROOT/m0/verify-m0.sh"
        "$ROOT/m1/verify-m1.sh"
        "$ROOT/m2/verify-m2.sh"
        ;;
    m5)
        "$ROOT/m5/verify-m5.sh"
        ;;
    *)
        echo "usage: $0 m0|m1|m2|m5" >&2
        exit 2
        ;;
esac
