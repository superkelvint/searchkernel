#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO=$(CDPATH= cd -- "$ROOT/../.." && pwd)
PYTHON=$(bash "$REPO/tools/resolve-python.sh")
"$PYTHON" "$ROOT/tools/validate_suite.py"
