#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "usage: $0 BASELINE_LIBRARY CURRENT_LIBRARY ABIXML_OUTPUT" >&2
    echo >&2
    echo "Compare two unstripped Search Kernel native ABI libraries and write" >&2
    echo "an ABIXML description of the current library." >&2
}

if [[ $# -ne 3 ]]; then
    usage
    exit 2
fi

baseline=$1
current=$2
abixml=$3

for tool in abidiff abidw; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "$tool is required for native ABI validation" >&2
        exit 127
    fi
done

for library in "$baseline" "$current"; do
    if [[ ! -f "$library" ]]; then
        echo "native ABI library does not exist: $library" >&2
        exit 1
    fi
done

mkdir -p "$(dirname "$abixml")"

# Keep the machine-readable ABI corpus even when the comparison reports an
# intentional change. It is useful for review and release diagnostics.
abidw --out-file "$abixml" "$current"

# --fail-no-debug-info is mandatory. Without it, a stripped or incorrectly
# packaged library can make the comparison silently degrade to symbols only.
abidiff --fail-no-debug-info "$baseline" "$current"

echo "native ABI comparison PASS"
