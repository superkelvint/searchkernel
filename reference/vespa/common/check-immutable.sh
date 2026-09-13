#!/usr/bin/env bash
set -euo pipefail

CHECKSUM="${1:?usage: check-immutable.sh CHECKSUM_FILE}"
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)"
[[ -s "$CHECKSUM" ]] || {
  echo "missing immutable verifier checksum: $CHECKSUM" >&2
  exit 1
}
(cd "$ROOT" && sha256sum --quiet --check "$CHECKSUM")
python3 - "$ROOT" "$CHECKSUM" <<'PY'
import sys
from pathlib import Path

root = Path(sys.argv[1])
checksum = Path(sys.argv[2]).resolve()
scope = checksum.parent
listed = {
    line.split(maxsplit=1)[1]
    for line in checksum.read_text(encoding="utf-8").splitlines()
    if line.strip()
}
for path in scope.rglob("*"):
    if not path.is_file() or path.resolve() == checksum:
        continue
    relative = path.relative_to(root)
    if "target" in relative.parts or "build" in relative.parts or "__pycache__" in relative.parts:
        continue
    if str(relative) not in listed:
        raise SystemExit(f"unlisted file added under immutable verifier scope: {relative}")
PY
