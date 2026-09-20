#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$DIR/../../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"

require_pinned_vespa
python3 "$DIR/validate.py"

# The low-level grouping case is independently compared with a direct pinned
# Vespa oracle and exercises the real native grouping ABI. M9 is frozen and is
# intentionally reused rather than modified by this additive M13 coverage.
"$ROOT/reference/vespa/m9-general-grouping-verifier/verify-recursive-grouping.sh"

# The portable showcase drives the daemon/native Vespa path through the shared
# FlatBuffer compiler. It covers the currently executable root/grouping and YQL
# cases. Future grouping work must make the manifest's blocked summary and
# continuation case executable without adding a second semantic implementation.
PYTHONPATH="$ROOT/clients/python${PYTHONPATH:+:$PYTHONPATH}" \
  python3 "$ROOT/clients/python/examples/try_searchkernel_client_api_e2e.py"

echo "M13 grouping/YQL acceptance coverage passed for currently executable cases"
