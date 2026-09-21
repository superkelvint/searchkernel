#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
expected=25.12.19
target_dir=${CARGO_TARGET_DIR:-$root/target/host}
if [[ "$target_dir" != /* ]]; then
    target_dir="$root/$target_dir"
fi

is_expected_flatc() {
    local candidate=$1
    [[ -x "$candidate" ]] &&
        [[ "$("$candidate" --version 2>/dev/null || true)" == "flatc version $expected" ]]
}

if [[ -n "${FLATC_PATH:-}" ]]; then
    if is_expected_flatc "$FLATC_PATH"; then
        printf '%s\n' "$FLATC_PATH"
        exit 0
    fi
    echo "error: FLATC_PATH='$FLATC_PATH' is not executable FlatBuffers $expected flatc" >&2
    exit 1
fi

common=$(git -C "$root" rev-parse --git-common-dir)
if [[ "$common" != /* ]]; then
    common="$root/$common"
fi
common=$(cd -- "$common" && pwd)
shared="$common/searchkernel/toolchains/flatbuffers/$expected/bin/flatc"
if is_expected_flatc "$shared"; then
    printf '%s\n' "$shared"
    exit 0
fi

if command -v flatc >/dev/null 2>&1; then
    candidate=$(command -v flatc)
    if is_expected_flatc "$candidate"; then
        printf '%s\n' "$candidate"
        exit 0
    fi
fi

candidate=$(find "$target_dir" -type f -path '*/flatbuffers-build-*/out/bin/flatc' \
    -perm -111 -print -quit 2>/dev/null || true)
if [[ -n "$candidate" ]] && is_expected_flatc "$candidate"; then
    printf '%s\n' "$candidate"
    exit 0
fi

while IFS= read -r worktree; do
    [[ "$worktree" == "$root" ]] && continue
    candidate=$(find "$worktree/target" -type f -path '*/flatbuffers-build-*/out/bin/flatc' \
        -perm -111 -print -quit 2>/dev/null || true)
    if [[ -n "$candidate" ]] && is_expected_flatc "$candidate"; then
        printf '%s\n' "$candidate"
        exit 0
    fi
done < <(git -C "$root" worktree list --porcelain | sed -n 's/^worktree //p')

if is_expected_flatc /usr/local/bin/flatc; then
    printf '%s\n' /usr/local/bin/flatc
    exit 0
fi

echo "error: FlatBuffers $expected flatc was not found" >&2
echo "       Run './dev install flatbuffers' once from any worktree; the versioned install is shared by the whole clone." >&2
exit 1
