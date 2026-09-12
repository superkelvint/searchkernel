#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
(cd "$ROOT" && sha256sum -c IMMUTABLE_SHA256SUMS >/dev/null)
