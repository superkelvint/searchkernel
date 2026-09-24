#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"

for script in install.sh package.sh verify.sh; do
    bash -n "$SCRIPT_DIR/$script"
done

help_output="$(bash "$SCRIPT_DIR/install.sh" --help)"
grep -Fq -- "--repo" <<<"$help_output"
grep -Fq -- "--token" <<<"$help_output"
grep -Fq -- "--runner-user" <<<"$help_output"

tmp="$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-runner-bootstrap-test.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

archive="$tmp/searchkernel-actions-runner-bootstrap.zip"
reported="$(bash "$SCRIPT_DIR/package.sh" "$archive")"
[[ "$reported" == "$archive" ]]
[[ -s "$archive" ]]

python3 - "$archive" <<'PY'
import stat
import sys
import zipfile

archive = sys.argv[1]
expected = [
    "README.md",
    "config.env.example",
    "install.sh",
    "verify.sh",
]

with zipfile.ZipFile(archive) as zf:
    names = sorted(zf.namelist())
    if names != sorted(expected):
        raise SystemExit(
            "runner bootstrap archive manifest mismatch:\n"
            f"expected={sorted(expected)!r}\nactual={names!r}"
        )

    for name in ("install.sh", "verify.sh"):
        mode = (zf.getinfo(name).external_attr >> 16) & 0o777
        if not mode & stat.S_IXUSR:
            raise SystemExit(f"{name} lost executable mode in bootstrap archive: {oct(mode)}")
PY

echo "runner bootstrap packaging tests PASS"
