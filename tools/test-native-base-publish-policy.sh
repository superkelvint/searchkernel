#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
PUBLISH=$ROOT/tools/publish-native-base.sh
BUILDER=$ROOT/tools/build-native-base-release.sh
VERIFY=$ROOT/tools/verify-native-base-archive.sh
REPAIR=$ROOT/tools/repair-native-base.sh
PACKAGE=$ROOT/tools/package-native-base-artifact.sh
REMOVE_READONLY_TREE=$ROOT/tools/remove-readonly-tree.sh
PYTHON_BIN=$(bash "$ROOT/tools/resolve-python.sh")

for script in "$PUBLISH" "$BUILDER" "$VERIFY" "$REPAIR" "$PACKAGE" "$REMOVE_READONLY_TREE"; do
    bash -n "$script"
done

grep -Fq 'GENERATION=$(' "$PUBLISH" || { echo 'publisher must allocate fresh release generations' >&2; exit 1; }
grep -Fq 'if gh release view "$TAG"' "$PUBLISH" || { echo 'publisher must reject existing immutable tags' >&2; exit 1; }
grep -Fq 'gh release create "$TAG"' "$PUBLISH" || { echo 'publisher must create a new draft release' >&2; exit 1; }
grep -Fq -- '--draft' "$PUBLISH" || { echo 'publisher must create drafts first' >&2; exit 1; }
grep -Fq '"$ROOT/dev" native-base build' "$PUBLISH" || { echo 'publisher must build through the canonical ./dev container path' >&2; exit 1; }
grep -Fq '"$ROOT/dev" native-base verify' "$PUBLISH" || { echo 'publisher must reverify downloaded bytes through the canonical ./dev container path' >&2; exit 1; }
if grep -Fq 'package-native-base-artifact.sh' "$PUBLISH" || grep -Fq 'bundle-native-runtime-closure.py' "$PUBLISH"; then
    echo 'publisher contains a competing artifact construction path' >&2
    exit 1
fi
if grep -Eq '\bgh\b|release (create|upload|edit)' "$BUILDER"; then
    echo 'build-only builder must not call GitHub' >&2
    exit 1
fi
if grep -Fq 'SEARCHKERNEL_DEV_USE_CI_IMAGE=0' "$BUILDER"; then
    echo 'native-base builder must not bypass canonical container execution' >&2
    exit 1
fi
grep -Fq "native-base release construction must run through './dev native-base build ...'" "$BUILDER" || { echo 'builder must fail closed outside the canonical container' >&2; exit 1; }
grep -Fq "native-base archive verification must run through './dev native-base verify ...'" "$VERIFY" || { echo 'archive verifier must fail closed outside the canonical container' >&2; exit 1; }
grep -Fq 'env -u SK_NATIVE_SDK' "$BUILDER" || { echo 'builder must ignore ambient SK_NATIVE_SDK' >&2; exit 1; }
grep -Fq 'check-native-runtime-closure.py' "$BUILDER" || { echo 'builder must verify ELF closure before archiving' >&2; exit 1; }
grep -Fq 'build-native-base-adapter.sh' "$BUILDER" || { echo 'builder must link the adapter against staged and archived bases' >&2; exit 1; }
grep -Fq 'smoke-native-base-adapter.py' "$BUILDER" || { echo 'builder must load and call the adapter ABI probe' >&2; exit 1; }
grep -Fq 'runtime_provenance_sha256' "$ROOT/tools/write-native-base-manifest.py" || { echo 'artifact manifest must seal runtime provenance' >&2; exit 1; }

builder_line=$(grep -n 'native-base build' "$PUBLISH" | head -1 | cut -d: -f1)
draft_line=$(grep -n 'gh release create' "$PUBLISH" | head -1 | cut -d: -f1)
download_line=$(grep -n 'gh release download' "$PUBLISH" | head -1 | cut -d: -f1)
remote_verify_line=$(grep -n 'native-base verify' "$PUBLISH" | tail -1 | cut -d: -f1)
publish_line=$(grep -n 'gh release edit' "$PUBLISH" | head -1 | cut -d: -f1)
(( builder_line < draft_line && draft_line < download_line && download_line < remote_verify_line && remote_verify_line < publish_line )) || {
    echo 'publisher order must be build, draft upload, download/check, consumer verification, publish' >&2
    exit 1
}

grep -Fq 'exec "$ROOT/tools/publish-native-base.sh" "$MODE"' "$REPAIR" || {
    echo 'repair must allocate a fresh generation through the canonical publisher' >&2
    exit 1
}
if grep -Fq -- '--clobber' "$REPAIR" || grep -Fq 'gh release upload' "$REPAIR"; then
    echo 'repair must not mutate an existing immutable generation' >&2
    exit 1
fi

"$PYTHON_BIN" "$ROOT/tools/test_native_base_metadata.py"
"$PYTHON_BIN" "$ROOT/tools/test_native_base_publish_mock.py"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-native-publish-policy.XXXXXX")
trap 'chmod -R u+w "$tmp" 2>/dev/null || true; rm -rf "$tmp"' EXIT
readonly_tree=$tmp/read-only-tree
mkdir -p "$readonly_tree/base/lib/nested"
printf 'fixture\n' >"$readonly_tree/base/lib/nested/runtime.so"
chmod -R a-w "$readonly_tree"
bash "$REMOVE_READONLY_TREE" "$readonly_tree"
[[ ! -e "$readonly_tree" ]] || { echo 'read-only native-base staging cleanup failed' >&2; exit 1; }

mkdir -p "$tmp/tools"
cp "$REPAIR" "$tmp/tools/repair-native-base.sh"
cat >"$tmp/tools/publish-native-base.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "${1:-}" >>"$TEST_PUBLISH_LOG"
SH
chmod +x "$tmp/tools/publish-native-base.sh"
export TEST_PUBLISH_LOG=$tmp/publish.log
for mode in ci host all; do
    : >"$TEST_PUBLISH_LOG"
    bash "$tmp/tools/repair-native-base.sh" "$mode" >/dev/null 2>&1
    [[ "$(cat "$TEST_PUBLISH_LOG")" == "$mode" ]] || { echo "repair mode $mode did not delegate" >&2; exit 1; }
done
: >"$TEST_PUBLISH_LOG"
if bash "$tmp/tools/repair-native-base.sh" invalid >"$tmp/invalid.out" 2>"$tmp/invalid.err"; then
    echo 'invalid repair mode unexpectedly succeeded' >&2
    exit 1
fi
grep -Fq 'usage:' "$tmp/invalid.err"
[[ ! -s "$TEST_PUBLISH_LOG" ]] || { echo 'invalid repair mode unexpectedly invoked publisher' >&2; exit 1; }

echo 'native-base publication immutability policy PASS'
