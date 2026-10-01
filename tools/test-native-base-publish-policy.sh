#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
PUBLISH="$ROOT/tools/publish-native-base.sh"
REPAIR="$ROOT/tools/repair-native-base.sh"
PACKAGE="$ROOT/tools/package-native-base-artifact.sh"
REMOVE_READONLY_TREE="$ROOT/tools/remove-readonly-tree.sh"

bash -n "$PUBLISH"
bash -n "$REPAIR"
bash -n "$PACKAGE"
bash -n "$REMOVE_READONLY_TREE"

for script in "$PUBLISH" "$PACKAGE"; do
    grep -Fq 'remove-readonly-tree.sh' "$script" || {
        echo "$script must clean staged read-only native-base files safely" >&2
        exit 1
    }
done

if grep -Fq -- "--clobber" "$REPAIR"; then
    echo "native-base repair must never clobber an existing published generation" >&2
    exit 1
fi

if grep -Fq "gh release upload" "$REPAIR"; then
    echo "native-base repair must not mutate assets on an existing release" >&2
    exit 1
fi

grep -Fq 'exec "$ROOT/tools/publish-native-base.sh" "$MODE"' "$REPAIR" || {
    echo "native-base repair must allocate a fresh generation through the canonical publisher" >&2
    exit 1
}

grep -Fq 'section "Determining next native-base generation"' "$PUBLISH" || {
    echo "native-base publisher must allocate the next generation" >&2
    exit 1
}
grep -Fq '! gh release view "$TAG"' "$PUBLISH" || {
    echo "native-base publisher must refuse an already-existing generation tag" >&2
    exit 1
}
grep -Fq 'gh release create "$TAG"' "$PUBLISH" || {
    echo "native-base publisher must create a fresh release identity" >&2
    exit 1
}
grep -Fq -- '--draft' "$PUBLISH" || {
    echo "native-base publisher must keep new generations hidden until verification" >&2
    exit 1
}

grep -Fq 'SEARCHKERNEL_DEV_USE_CI_IMAGE=0' "$PUBLISH" || {
    echo "host native-base publication must provision in the selected host environment" >&2
    exit 1
}
grep -Fq 'env -u SK_NATIVE_SDK' "$PUBLISH" || {
    echo "native-base publication must ignore ambient legacy SK_NATIVE_SDK overrides" >&2
    exit 1
}
grep -Fq 'provision_native_base "$BUILD_ENV"' "$PUBLISH" || {
    echo "native-base publication must provision before packaging each selected environment" >&2
    exit 1
}

grep -Fq 'normalize_absolute_root' "$PACKAGE" || {
    echo "native-base packager must normalize producer roots without corrupting relative CMake paths" >&2
    exit 1
}
if grep -Fq 'text.replace(vespa_root, "/work/vespa")' "$PACKAGE"; then
    echo "native-base packager must not blindly replace the Vespa root substring" >&2
    exit 1
fi

python3 - "$PUBLISH" <<'PY'
import pathlib
import sys

text = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
draft_upload = text.index('gh release create "$TAG"')
remote_verify = text.index('section "Remote draft verification:')
publish_verified = text.index('section "Publishing verified native-base releases"')
if not (draft_upload < remote_verify < publish_verified):
    raise SystemExit(
        "native-base generation must remain draft through remote verification"
    )
PY

tmp="$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-native-publish-policy.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

readonly_tree="$tmp/read-only-tree"
mkdir -p "$readonly_tree/base/lib/nested"
printf 'fixture\n' >"$readonly_tree/base/lib/nested/runtime.so"
chmod -R a-w "$readonly_tree"
bash "$REMOVE_READONLY_TREE" "$readonly_tree"
[[ ! -e "$readonly_tree" ]] || {
    echo "read-only native-base staging tree was not removed" >&2
    exit 1
}

mkdir -p "$tmp/tools"
cp "$REPAIR" "$tmp/tools/repair-native-base.sh"
cat >"$tmp/tools/publish-native-base.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "${1:-}" >>"$TEST_PUBLISH_LOG"
SH
chmod +x "$tmp/tools/publish-native-base.sh"

export TEST_PUBLISH_LOG="$tmp/publish.log"
for mode in ci host all; do
    : >"$TEST_PUBLISH_LOG"
    bash "$tmp/tools/repair-native-base.sh" "$mode" >/dev/null 2>&1
    [[ "$(cat "$TEST_PUBLISH_LOG")" == "$mode" ]] || {
        echo "repair mode $mode did not delegate unchanged to the canonical publisher" >&2
        exit 1
    }
done

: >"$TEST_PUBLISH_LOG"
if bash "$tmp/tools/repair-native-base.sh" invalid >"$tmp/invalid.out" 2>"$tmp/invalid.err"; then
    echo "invalid repair mode unexpectedly succeeded" >&2
    exit 1
fi
grep -Fq "usage:" "$tmp/invalid.err"
[[ ! -s "$TEST_PUBLISH_LOG" ]] || {
    echo "invalid repair mode must fail before invoking the publisher" >&2
    exit 1
}

echo "native-base publication immutability policy PASS"
