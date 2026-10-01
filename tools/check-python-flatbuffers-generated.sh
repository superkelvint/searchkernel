#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT
python=$(bash "$root/tools/resolve-python.sh")

fixture_root="$root/core/crates/searchkernel-dispatch/tests/fixtures/portable"
fixture_hash_before=$("$python" - "$fixture_root" <<'PY'
from __future__ import annotations
import hashlib
import sys
from pathlib import Path
root = Path(sys.argv[1])
digest = hashlib.sha256()
for path in sorted([*root.rglob("*.skrq"), *root.rglob("*.skrs")]):
    digest.update(path.relative_to(root).as_posix().encode())
    digest.update(path.read_bytes())
print(digest.hexdigest())
PY
)

PYTHON_FLATBUFFERS_GENERATED_DIR="$temporary/generated"   bash "$root/tools/generate-python-flatbuffers.sh"

diff -ruNB --exclude='__pycache__' --exclude='*.pyc' \
    "$root/clients/python/searchkernel/_wire/v1/generated" \
    "$temporary/generated"

fixture_hash_after=$("$python" - "$fixture_root" <<'PY'
from __future__ import annotations
import hashlib
import sys
from pathlib import Path
root = Path(sys.argv[1])
digest = hashlib.sha256()
for path in sorted([*root.rglob("*.skrq"), *root.rglob("*.skrs")]):
    digest.update(path.relative_to(root).as_posix().encode())
    digest.update(path.read_bytes())
print(digest.hexdigest())
PY
)
[[ "$fixture_hash_before" == "$fixture_hash_after" ]] || {
    echo "Python FlatBuffers generation mutated canonical fixtures" >&2
    exit 1
}

echo "Python FlatBuffers generation reproducibility PASS"
