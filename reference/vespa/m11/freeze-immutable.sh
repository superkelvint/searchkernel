#!/usr/bin/env bash
set -euo pipefail

M11="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M11/../../.." && pwd)"
CHECKSUM="$M11/M11-IMMUTABLE.sha256"

paths=()
while IFS= read -r -d '' path; do
  paths+=("${path#"$ROOT/"}")
done < <(find "$M11" -type f \
  ! -path "$M11/M11-IMMUTABLE.sha256" \
  ! -path '*/target/*' ! -path '*/build/*' ! -path '*/__pycache__/*' -print0 | sort -z)
paths+=("reference/vespa/api-contract/src/bin/m11.rs")
paths+=("reference/vespa/verify-through.sh")
(cd "$ROOT" && sha256sum "${paths[@]}") > "$CHECKSUM"
"$M11/check-immutable.sh"
