#!/usr/bin/env bash
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

case "${1:-}" in
    m0)
        "$ROOT/m0/verify-m0.sh"
        ;;
    m1|m2|m3|m4|m5|m6|m7|m8|m9|m10|m11|m12|m13)
        "$ROOT/$1/verify-$1.sh"
        ;;
    searchreply)
        "$ROOT/searchreply/verify-searchreply.sh"
        ;;
    phase10)
        "$ROOT/phase10/verify-phase10.sh"
        ;;
    i)
        "$ROOT/i/verify-i.sh"
        ;;
    *)
        echo "usage: $0 m0|m1|m2|m3|m4|m5|m6|m7|m8|m9|m10|m11|m12|m13|searchreply|phase10|i" >&2
        exit 2
        ;;
esac
