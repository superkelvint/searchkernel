#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"

run_cargo() {
    if [[ -n "${SK_NATIVE_SDK:-}" ]]; then
        LD_LIBRARY_PATH="$SK_NATIVE_SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
            cargo "$@"
    else
        cargo "$@"
    fi
}

# The C++ verifier translation units are compiled only by the explicit
# contract-test feature. Ordinary bindings builds validate and link the
# packaged SDK without invoking a C++ compiler for these helpers.
run_cargo test -p searchkernel-vespa-bindings --features native-contract-tests --lib
run_cargo test -p searchkernel-vespa-engine --test native_contract --features native-contract-tests
