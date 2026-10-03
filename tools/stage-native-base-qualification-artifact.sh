#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "usage: $0 <output-dir>" >&2
    exit 2
fi

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
PYTHON_BIN=$(bash "$ROOT/tools/resolve-python.sh")
RUSTC_BIN=$(bash "$ROOT/tools/resolve-rust.sh" rustc)
OUTPUT=$(mkdir -p "$1" && cd "$1" && pwd -P)
BUILD_ENV=${SEARCHKERNEL_BUILD_ENV:-host}
TARGET=${SK_NATIVE_TARGET:-$("$RUSTC_BIN" -vV | sed -n 's/^host: //p')}
PINNED_VESPA_COMMIT=$("$PYTHON_BIN" - "$ROOT/core/native/vespa/manifest.json" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as stream:
    print(json.load(stream)["vespa_commit"])
PY
)

[[ -n "$TARGET" ]] || { echo "could not determine native target" >&2; exit 1; }
case "$BUILD_ENV" in
    host|ci) ;;
    *) echo "unsupported native-base qualification environment: $BUILD_ENV" >&2; exit 2 ;;
esac

BASE=$(bash "$ROOT/tools/resolve-native-sdk.sh" path)
MARKER="$BASE/.searchkernel-native-base-artifact.json"
[[ -f "$MARKER" ]] || {
    echo "installed native base lacks release metadata: $MARKER" >&2
    exit 1
}
GENERATION=$("$PYTHON_BIN" - "$MARKER" "$BUILD_ENV" "$TARGET" "$PINNED_VESPA_COMMIT" <<'PY'
import json
import sys

path, build_env, target, commit = sys.argv[1:]
with open(path, encoding="utf-8") as stream:
    marker = json.load(stream)
expected = {
    "build_environment": build_env,
    "native_target": target,
    "vespa_commit": commit,
}
for key, value in expected.items():
    if marker.get(key) != value:
        raise SystemExit(
            f"installed native-base {key} mismatch: expected {value!r}, found {marker.get(key)!r}"
        )
generation = marker.get("artifact_generation", marker.get("format_version"))
if not isinstance(generation, int) or generation < 1:
    raise SystemExit("installed native base has no valid artifact generation")
print(generation)
PY
)

if find "$OUTPUT" -mindepth 1 -print -quit | grep -q .; then
    echo "native-base qualification output must be empty: $OUTPUT" >&2
    exit 1
fi

TMP=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-native-base-qualification.XXXXXX")
cleanup() {
    local status=$?
    trap - EXIT INT TERM
    bash "$ROOT/tools/remove-readonly-tree.sh" "$TMP" >/dev/null 2>&1 || true
    exit "$status"
}
trap cleanup EXIT INT TERM

STAGE="$TMP/stage"
env     SEARCHKERNEL_BUILD_ENV="$BUILD_ENV"     SK_NATIVE_TARGET="$TARGET"     SK_NATIVE_BASE_GENERATION="$GENERATION"     SK_NATIVE_BASE_SOURCE_COMMIT="$(git -C "$ROOT" rev-parse HEAD)"     bash "$ROOT/tools/package-native-base-artifact.sh" "$OUTPUT" "$STAGE" >/dev/null

ASSET="searchkernel-vespa-base-v${GENERATION}-${BUILD_ENV}-${TARGET}-${PINNED_VESPA_COMMIT}.tar.gz"
ARCHIVE="$OUTPUT/$ASSET"
(
    cd "$STAGE"
    tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0         --numeric-owner -cf - searchkernel-vespa-base | gzip -n >"$ARCHIVE"
)
sha256sum "$ARCHIVE" >"$ARCHIVE.sha256"

"$PYTHON_BIN" - "$OUTPUT/manifest.json" "$ASSET" "$BUILD_ENV" "$TARGET" "$PINNED_VESPA_COMMIT" "$GENERATION" "$ARCHIVE" <<'PY'
import hashlib
import json
import pathlib
import sys

out, asset, build_env, target, commit, generation, archive = sys.argv[1:]
path = pathlib.Path(archive)
payload = {
    "artifact": asset,
    "build_environment": build_env,
    "native_target": target,
    "vespa_commit": commit,
    "artifact_generation": int(generation),
    "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    "bytes": path.stat().st_size,
}
pathlib.Path(out).write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
PY

printf '%s\n' "$ARCHIVE"
