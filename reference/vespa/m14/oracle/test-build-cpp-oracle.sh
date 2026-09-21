#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# Keep this as a real oracle build smoke test: the source is compiled with the
# pinned Vespa warning policy, including -Werror, rather than checked by text.
"$SCRIPT_DIR/build-cpp-oracle.sh"
