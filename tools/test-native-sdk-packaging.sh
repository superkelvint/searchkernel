#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
helper="$root/tools/copy-shared-library-into-sdk.sh"
fixture=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-sdk-package.XXXXXX")
trap 'rm -rf "$fixture"' EXIT

src="$fixture/src"
dest="$fixture/sdk/lib"
external="$fixture/external"
mkdir -p "$src" "$dest" "$external"

cat >"$fixture/fixture.c" <<'C'
int searchkernel_packaging_fixture(void) { return 7; }
C
cc -shared -fPIC "$fixture/fixture.c" -Wl,-soname,libpackaging_fixture.so.0 \
    -o "$external/libpackaging_fixture.so.0.1"
ln -s "$external/libpackaging_fixture.so.0.1" "$src/libpackaging_fixture.so"

# A stale destination symlink must be replaced, never followed.
printf 'do-not-overwrite\n' >"$external/victim"
ln -s "$external/victim" "$dest/libpackaging_fixture.so.0"

sh "$helper" "$dest" "$src/libpackaging_fixture.so"

[[ -f "$dest/libpackaging_fixture.so" && ! -L "$dest/libpackaging_fixture.so" ]]
[[ -f "$dest/libpackaging_fixture.so.0" && ! -L "$dest/libpackaging_fixture.so.0" ]]
[[ "$(cat "$external/victim")" == 'do-not-overwrite' ]]
cmp "$dest/libpackaging_fixture.so" "$external/libpackaging_fixture.so.0.1"
cmp "$dest/libpackaging_fixture.so.0" "$external/libpackaging_fixture.so.0.1"

cat >"$fixture/consumer.c" <<'C'
extern int searchkernel_packaging_fixture(void);
int call_fixture(void) { return searchkernel_packaging_fixture(); }
C
cc -shared -fPIC "$fixture/consumer.c" -L"$src" -lpackaging_fixture \
    -o "$fixture/libconsumer.so"
resolved=$(LD_LIBRARY_PATH="$dest" ldd "$fixture/libconsumer.so" |
    awk '$1 == "libpackaging_fixture.so.0" {print $3}')
[[ "$resolved" == "$dest/libpackaging_fixture.so.0" ]]

# DT_SONAME is a package filename, not a path. Reject path traversal/absolute
# names rather than allowing an ELF to write outside SDK/lib.
cc -shared -fPIC "$fixture/fixture.c" -Wl,-soname,../escape.so \
    -o "$src/libbad_soname.so"
if sh "$helper" "$dest" "$src/libbad_soname.so" >"$fixture/bad.out" 2>"$fixture/bad.err"; then
    echo 'packager unexpectedly accepted a path-like DT_SONAME' >&2
    exit 1
fi
grep -q 'invalid shared-library package name' "$fixture/bad.err"
[[ ! -e "$fixture/sdk/escape.so" ]]

echo 'native SDK packaging regression PASS'
