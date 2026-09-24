#!/usr/bin/env bash
set -euo pipefail
trap 'echo "native-base fetch regression failed at line $LINENO: $BASH_COMMAND" >&2' ERR

source_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "x86_64" ]]; then
    echo "native-base fetch regression SKIP: Linux x86_64 fallback only"
    exit 0
fi

fixture=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-fetch-native-base.XXXXXX")
cleanup() {
    rm -rf "$fixture"
}
trap cleanup EXIT

repo="$fixture/repo"
fakebin="$fixture/fakebin"
mkdir -p "$repo/tools" "$repo/core/native/vespa" "$fakebin" "$fixture/home"
cp "$source_root/tools/fetch-native-base.sh" "$repo/tools/fetch-native-base.sh"
chmod +x "$repo/tools/fetch-native-base.sh"

pin=2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
printf '{"vespa_commit":"%s"}\n' "$pin" >"$repo/core/native/vespa/manifest.json"
git -C "$repo" init -q

for cmd in git sha256sum sed awk mkdir mktemp mv rm uname; do
    command_path=$(command -v "$cmd")
    ln -s "$command_path" "$fakebin/$cmd"
done

real_python=$(command -v python3)
cat >"$fakebin/python3" <<PY
#!/bin/bash
set -euo pipefail
if [[ "\${1:-}" == */native-base-release-selection.py ]]; then
    printf '3\\n'
    exit 0
fi
exec "$real_python" "\$@"
PY
chmod +x "$fakebin/python3"

cat >"$fakebin/gh" <<'GH'
#!/bin/bash
set -euo pipefail

pin=2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
target=x86_64-unknown-linux-gnu
tag="vespa-base-v3-ci-${target}-${pin}"
asset="searchkernel-vespa-base-v3-ci-${target}-${pin}.tar.gz"

if [[ "${1:-}" == "auth" && "${2:-}" == "token" ]]; then
    printf 'fixture-token\\n'
    exit 0
fi

if [[ "${1:-}" == "api" ]]; then
    for arg in "$@"; do
        if [[ "$arg" == *"/releases?per_page=100" ]]; then
            printf '%s\n' "$tag"
            exit 0
        fi
    done
    exit 0
fi

if [[ "${1:-}" == "release" && "${2:-}" == "download" ]]; then
    destination=
    shift 2
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -D)
                destination=$2
                shift 2
                ;;
            *)
                shift
                ;;
        esac
    done
    [[ -n "$destination" ]]
    printf 'fixture native base\n' >"$destination/$asset"
    /usr/bin/sha256sum "$destination/$asset" >"$destination/$asset.sha256"
    exit 0
fi

echo "unexpected gh invocation: $*" >&2
exit 2
GH
chmod +x "$fakebin/gh"

stderr="$fixture/fetch.err"
if ! artifact=$(
    cd "$repo"
    env -u SK_NATIVE_TARGET \
        PATH="$fakebin" \
        HOME="$fixture/home" \
        XDG_CACHE_HOME="$fixture/cache" \
        SEARCHKERNEL_GITHUB_REPO=superkelvint/searchkernel \
        /bin/bash "$repo/tools/fetch-native-base.sh" ci
) 2>"$stderr"; then
    cat "$stderr" >&2
    exit 1
fi

expected="$fixture/cache/searchkernel/native-artifacts/searchkernel-vespa-base-v3-ci-x86_64-unknown-linux-gnu-${pin}.tar.gz"
[[ "$artifact" == "$expected" ]]
[[ -s "$artifact" ]]
[[ -s "$artifact.sha256" ]]

echo "native-base fetch without rustc regression PASS"
