#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
[ $# -eq 1 ] || { echo 'usage: verify-adapter.sh /path/to/adapter' >&2; exit 2; }
python3 "$ROOT/tools/verify_adapter.py" "$1"
