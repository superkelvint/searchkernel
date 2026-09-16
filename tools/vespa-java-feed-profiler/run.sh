#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="${SCRIPT_DIR}/target"
CLASS_DIR="${BUILD_DIR}/classes"

mkdir -p "$CLASS_DIR"
javac --release 11 -Xlint:all -Werror -d "$CLASS_DIR" \
    "$SCRIPT_DIR/src/VespaFeedProfiler.java"
exec java -cp "$CLASS_DIR" VespaFeedProfiler "$@"
