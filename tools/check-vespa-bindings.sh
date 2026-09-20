#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec bash "$ROOT/tools/check-native-bindings.sh"
