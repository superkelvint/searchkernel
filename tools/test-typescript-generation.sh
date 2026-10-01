#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
node=$(bash "$root/tools/resolve-node.sh" node)
PATH="$(dirname -- "$node"):$PATH"
export PATH
npm=$(bash "$root/tools/resolve-node.sh" npm)
client="$root/clients/typescript"
generated="$client/src/_wire/v1/generated/flatbuffers"
backup=$(mktemp -d)
failures=0

cleanup() {
    rm -rf -- "$generated"
    if [[ -d "$backup/generated" ]]; then
        mv -- "$backup/generated" "$generated"
    fi
    rmdir --ignore-fail-on-non-empty "$backup"
}
trap cleanup EXIT

if [[ -e "$generated" ]]; then
    mv -- "$generated" "$backup/generated"
fi

if ! "$npm" run build --prefix "$client"; then
    echo 'TypeScript clean-source build failed without generated output' >&2
    failures=$((failures + 1))
fi

"$npm" run generate --prefix "$client"
probe=$(find "$generated" -type f -name '*.ts' -print -quit)
if [[ -z "$probe" ]]; then
    echo 'TypeScript generation produced no TypeScript files' >&2
    failures=$((failures + 1))
else
    printf '\n// stale generated-output probe\n' >> "$probe"
    if "$npm" run check:generated --prefix "$client" >/dev/null 2>&1; then
        echo 'TypeScript freshness check accepted perturbed generated output' >&2
        failures=$((failures + 1))
    fi
fi

exit "$failures"
