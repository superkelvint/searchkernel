#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

required=(
    core/crates/searchkernel
    core/crates/searchkernel-protocol
    core/crates/searchkernel-rpc
    core/crates/searchkernel-ir
    core/crates/searchkernel-dispatch
    core/crates/searchkernel-testkit
    core/crates/searchkernel-ir-adapter
    core/crates/searchkernel-vespa-engine
    core/crates/searchkernel-vespa-bindings
    core/native/vespa
)

legacy=(
    crates
    native
    core/crates/searchkernel-api
    core/crates/searchkernel-core
    core/crates/searchkernel-vespa
    core/crates/searchkernel-vespa-native
    core/crates/searchkernel-vespa-sys
    core/crates/searchkernel-sys
)

for path in "${required[@]}"; do
    [[ -d "$ROOT/$path" ]] || {
        echo "missing required layout path: $path" >&2
        exit 1
    }
done

for path in "${legacy[@]}"; do
    if [[ -e "$ROOT/$path" || -L "$ROOT/$path" ]]; then
        echo "legacy layout path remains: $path" >&2
        exit 1
    fi
done

echo "repository layout PASS"
