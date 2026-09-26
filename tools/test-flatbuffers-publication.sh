#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
PUBLISH="$ROOT/tools/publish-flatbuffers-toolchain.sh"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-flatbuffers-publish.XXXXXX")
trap 'rm -rf -- "$tmp"' EXIT

old="$tmp/old"
candidate="$tmp/candidate"
published="$tmp/current"
mkdir -p "$old" "$candidate"
printf 'old\n' >"$old/marker"
printf 'new\n' >"$candidate/marker"
ln -s "$old" "$published"

bash "$PUBLISH" "$candidate" "$published"

[[ "$(cat "$published/marker")" == "new" ]]
[[ -L "$published" ]]

echo "PASS FlatBuffers publication swaps complete generations"
