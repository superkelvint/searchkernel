#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
python3 "$ROOT/tools/validate_suite.py"
(cd "$ROOT" && sha256sum -c IMMUTABLE_SHA256SUMS)
