#!/usr/bin/env bash
set -euo pipefail
M12="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M12/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa
"$M12/validate_scenarios.py"
[[ -x "$M12/oracle/target/m12_cpp_oracle" ]] || fail_verification "build/review the M12 direct C++ oracle before freezing goldens"
mkdir -p "$M12/expected"
rm -f "$M12"/expected/*.json
for s in "$M12"/scenarios/[0-9][0-9]-*.json; do
  id="$(python3 "$ROOT/tools/json-cli.py" get "$s" id)"
  echo "M12 freezing $id" >&2
  "$M12/oracle/run-cpp-oracle.sh" "$s" | "$M12/canonicalize.py" > "$M12/expected/$id.json"
done
"$M12/validate_expected.py"
echo 'M12 goldens frozen from direct pinned Vespa C++ oracle'
