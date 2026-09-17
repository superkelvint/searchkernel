#!/usr/bin/env bash
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

case "${1:-}" in
    m0)
        "$ROOT/m0/verify-m0.sh"
        ;;
    m1|m2|m3|m4|m5|m6|m7|m8|m9|m10|m11)
        "$ROOT/$1/verify-$1.sh"
        ;;
    i)
        "$ROOT/i/verify-i.sh"
        ;;
    c7)
        "$ROOT/../clients/c7/verify-c7.sh"
        ;;
    *)
        echo "usage: $0 m0|m1|m2|m3|m4|m5|m6|m7|m8|m9|m10|m11|i|c7" >&2
        exit 2
        ;;
esac
