#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M4="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="$(cd "$M4/../../.." && pwd)"
BIN="$M4/oracle/target/m4_cpp_oracle"
IMAGE="${M4_VESPA_DEV_IMAGE:-docker.io/vespaengine/vespa-dev-almalinux-8:latest}"

"$M4/oracle/prepare-fixture.sh" >&2

[[ -x "$BIN" ]] || {
  cat >&2 <<EOF
M4 C++ oracle binary is missing:

  $BIN

Build it once inside the pinned Vespa Docker build environment.
freeze-goldens.sh never builds native code.
EOF
  exit 1
}

# If already running in the Vespa build container, keep all Proton runtime
# state in container-local /tmp. Never write engine state into the repository.
if [[ -f /.dockerenv ]]; then
  WORK="$(mktemp -d /tmp/searchkernel-m4-oracle.XXXXXX)"
  trap 'rm -rf "$WORK"' EXIT
  cd "$WORK"
  export M4_ORACLE_CONFIG_DIR="$M4/fixture"
  exec "$BIN"
fi

# Host path:
#
# Mount the repository read-only. Proton's mutable runtime state lives only
# inside this ephemeral container under /tmp, so Docker can never leave
# root-owned index files in the host checkout.
exec docker run --rm -i \
  --privileged \
  --pids-limit -1 \
  -v "$ROOT":/work:ro \
  -e M4_ORACLE_CONFIG_DIR=/work/reference/vespa/m4/fixture \
  "$IMAGE" \
  /bin/bash -lc '
    set -e
    WORK="$(mktemp -d /tmp/searchkernel-m4-oracle.XXXXXX)"
    cd "$WORK"
    exec /work/reference/vespa/m4/oracle/target/m4_cpp_oracle
  '
