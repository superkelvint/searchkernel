#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
fixture="$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-verifier-provenance.XXXXXX")"
cleanup() { rm -rf "$fixture"; }
trap cleanup EXIT

published="$fixture/toolchain/upstream/vespa"
configured="$published/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir"
mkdir -p "$configured" "$fixture/toolchain/vespa-deps/include" "$fixture/sdk"
printf 'link\n' >"$configured/link.txt"
printf 'CXX_INCLUDES = -I/work/vespa\n' >"$configured/flags.make"

pin="$fixture/PINNED_VESPA.json"
printf '{"commit":"fixture-vespa-commit"}\n' >"$pin"
marker="$published/.searchkernel-vespa-artifact.json"
write_marker() {
  local commit=$1
  local closure=$2
  python3 - "$marker" "$commit" "$closure" <<'PY'
import json
import sys
path, commit, closure = sys.argv[1:]
data = {
    "format_version": 2,
    "artifact_generation": 7,
    "adapter_static_link_closure": closure == "true",
    "build_environment": "verifier-regression",
    "native_target": "x86_64-unknown-linux-gnu",
    "vespa_commit": commit,
}
with open(path, "w", encoding="utf-8") as stream:
    json.dump(data, stream, sort_keys=True)
    stream.write("\n")
PY
}
write_marker fixture-vespa-commit true

export SK_NATIVE_SDK="$fixture/sdk"
export VESPA_ROOT="$published"
export SEARCHKERNEL_BUILD_ENV=verifier-regression
export SK_NATIVE_TARGET=x86_64-unknown-linux-gnu
# shellcheck source=reference/vespa/common/verify-common.sh
source "$ROOT/reference/vespa/common/verify-common.sh"
PIN_FILE="$pin"

require_pinned_vespa
if live_oracle_enabled; then
  echo "published toolchain unexpectedly selected live upstream oracle mode" >&2
  exit 1
fi
if (
  export SEARCHKERNEL_VERIFY_LIVE_ORACLE=required
  live_oracle_enabled
) >/dev/null 2>&1; then
  echo "published toolchain unexpectedly allowed required live-oracle mode" >&2
  exit 1
fi

write_marker wrong-commit true
if (require_pinned_vespa) >/dev/null 2>&1; then
  echo "published verifier provenance accepted the wrong Vespa commit" >&2
  exit 1
fi

write_marker fixture-vespa-commit false
if (require_pinned_vespa) >/dev/null 2>&1; then
  echo "published verifier provenance accepted missing static-link closure certification" >&2
  exit 1
fi

write_marker fixture-vespa-commit true
rm "$configured/link.txt"
if (require_pinned_vespa) >/dev/null 2>&1; then
  echo "published verifier provenance accepted missing adapter link metadata" >&2
  exit 1
fi
printf 'link\n' >"$configured/link.txt"

# Source-checkout qualification remains stricter: exact pinned HEAD and a clean
# worktree are still mandatory when live upstream oracles are available.
source_vespa="$fixture/source-vespa"
mkdir -p "$source_vespa"
git -C "$source_vespa" init -q
git -C "$source_vespa" config user.email verifier@example.com
git -C "$source_vespa" config user.name 'Verifier Provenance Test'
printf 'pinned\n' >"$source_vespa/tracked.txt"
git -C "$source_vespa" add tracked.txt
git -C "$source_vespa" commit -qm pinned
source_commit="$(git -C "$source_vespa" rev-parse HEAD)"
printf '{"commit":"%s"}\n' "$source_commit" >"$pin"

VESPA_ROOT="$source_vespa"
export VESPA_ROOT
require_pinned_vespa
if ! live_oracle_enabled; then
  echo "clean pinned source checkout did not select live-oracle qualification mode" >&2
  exit 1
fi

printf 'dirty\n' >>"$source_vespa/tracked.txt"
if (require_pinned_vespa) >/dev/null 2>&1; then
  echo "source-checkout verifier provenance accepted a dirty Vespa tree" >&2
  exit 1
fi

echo "published verifier provenance regression PASS"
