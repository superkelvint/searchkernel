#!/usr/bin/env bash

# Shared acceptance helpers. Every milestone verifier sources this file so the
# acceptance rules do not drift between milestones.

COMMON_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REFERENCE_ROOT="$(cd "$COMMON_DIR/.." && pwd)"
REPO_ROOT="$(cd "$REFERENCE_ROOT/../.." && pwd)"
PIN_FILE="$REFERENCE_ROOT/PINNED_VESPA.json"
# ./dev supplies its worktree-scoped Cargo target root. Direct supported host
# calls fall back to BUILDING.md's normal repository target, not a verifier-owned tree.
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target/host}"
SK_NATIVE_SDK_WAS_EXPLICIT=0
if [[ -n "${SK_NATIVE_SDK:-}" ]]; then
  SK_NATIVE_SDK_WAS_EXPLICIT=1
  SDK="$SK_NATIVE_SDK"
  if [[ -d "$SDK" ]]; then
    SDK="$(cd "$SDK" && pwd -P)"
  fi
  export SK_NATIVE_SDK="$SDK"
else
  # Sourcing this helper must never compile, link, or mutate verifier state.
  # Resolve the immutable inputs and native-probe output paths only. Normal Rust
  # examples use Cargo's shared native-base resolver. Direct native
  # probes opt into prepare_native_probe_sdk explicitly below.
  unset SK_NATIVE_SDK
  BASE_SDK="${SEARCHKERNEL_NATIVE_PROBE_BASE_SDK:-$(bash "$REPO_ROOT/tools/resolve-native-sdk.sh" path)}"
  VESPA_ROOT="${SEARCHKERNEL_NATIVE_PROBE_VESPA_ROOT:-$(bash "$REPO_ROOT/tools/resolve-native-sdk.sh" vespa-root)}"
  SDK="${SEARCHKERNEL_NATIVE_PROBE_SDK:-$CARGO_TARGET_DIR/native-probe-sdk}"
  SDK_BUILD="${SEARCHKERNEL_NATIVE_PROBE_SDK_BUILD:-$CARGO_TARGET_DIR/native-probe-sdk-build}"
fi
if [[ "$SK_NATIVE_SDK_WAS_EXPLICIT" == 1 ]]; then
  SK_NATIVE_SDK="$SDK"
  export SK_NATIVE_SDK
else
  # The native-probe SDK is for direct probes and fixtures. Cargo must stay
  # on the shared-base path so build.rs creates its revision-local adapter and
  # never mistakes this adapter-only verifier view for a full explicit SDK.
  unset SK_NATIVE_SDK
fi
# Normal Cargo builds validate the checked-in source header and create their
# adapter in Cargo's target output. Probe-only SDK paths must not leak into
# ordinary Rust verifier builds through SK_NATIVE_ABI_HEADER.
if [[ -z "${VESPA_ROOT:-}" ]]; then
  VESPA_ROOT=$(bash "$REPO_ROOT/tools/resolve-native-sdk.sh" vespa-root)
fi
export VESPA_ROOT
export CARGO_TARGET_DIR

native_probe_sdk_cache_valid() {
  [[ -f "$SDK/manifest.json" ]] || return 1
  [[ -f "$SDK/include/searchkernel_vespa.h" ]] || return 1
  [[ -f "$SDK/lib/libsearchkernel_vespa.so" ]] || return 1
  [[ -f "$SDK/native-build-fingerprint-v2.txt" ]] || return 1

  local native_target expected python_bin rustc_bin
  python_bin="$(bash "$REPO_ROOT/tools/resolve-python.sh")" || return 1
  rustc_bin="$(bash "$REPO_ROOT/tools/resolve-rust.sh" rustc)" || return 1
  native_target="${SK_NATIVE_TARGET:-$("$rustc_bin" -vV | sed -n 's/^host: //p')}"
  [[ -n "$native_target" ]] || return 1
  expected="$(mktemp "${TMPDIR:-/tmp}/searchkernel-native-probe-fingerprint.XXXXXX")"
  if ! RUSTC="$rustc_bin" "$python_bin" "$REPO_ROOT/tools/native-build-fingerprint.py" \
      --root "$REPO_ROOT" \
      --vespa-root "$VESPA_ROOT" \
      --base-sdk "$BASE_SDK" \
      --target "$native_target" \
      --scope full \
      --output "$expected"; then
    rm -f "$expected"
    return 1
  fi
  if cmp -s "$expected" "$SDK/native-build-fingerprint-v2.txt"; then
    rm -f "$expected"
    return 0
  fi
  rm -f "$expected"
  return 1
}

prepare_native_probe_sdk() {
  if [[ "$SK_NATIVE_SDK_WAS_EXPLICIT" == 1 ]]; then
    return 0
  fi

  if [[ "${SEARCHKERNEL_NATIVE_PROBE_SDK_PREPARED:-0}" == 1 ]]; then
    [[ -f "$SDK/manifest.json" ]] || {
      echo "verification error: prepared native-probe SDK manifest is missing: $SDK/manifest.json" >&2
      return 1
    }
    [[ -f "$SDK/include/searchkernel_vespa.h" ]] || {
      echo "verification error: prepared native-probe SDK header is missing: $SDK/include/searchkernel_vespa.h" >&2
      return 1
    }
    return 0
  fi

  if native_probe_sdk_cache_valid; then
    echo "reusing Search Kernel native-probe SDK: $SDK" >&2
  else
    echo "preparing Search Kernel native-probe SDK: $SDK" >&2
    SK_NATIVE_SDK="$SDK" \
    SK_NATIVE_BUILD_DIR="$SDK_BUILD" \
    SK_NATIVE_BASE_SDK="$BASE_SDK" \
    SK_NATIVE_ADAPTER_ONLY=1 \
    SK_NATIVE_REGENERATE_BINDINGS=0 \
    SK_NATIVE_SKIP_BINDINGS_CHECK=1 \
    VESPA_ROOT="$VESPA_ROOT" \
      bash "$REPO_ROOT/core/native/vespa/build-sdk.sh" >/dev/null
  fi

  SEARCHKERNEL_NATIVE_PROBE_SDK="$SDK"
  SEARCHKERNEL_NATIVE_PROBE_SDK_BUILD="$SDK_BUILD"
  SEARCHKERNEL_NATIVE_PROBE_BASE_SDK="$BASE_SDK"
  SEARCHKERNEL_NATIVE_PROBE_VESPA_ROOT="$VESPA_ROOT"
  SEARCHKERNEL_NATIVE_PROBE_SDK_PREPARED=1
  export SEARCHKERNEL_NATIVE_PROBE_SDK
  export SEARCHKERNEL_NATIVE_PROBE_SDK_BUILD
  export SEARCHKERNEL_NATIVE_PROBE_BASE_SDK
  export SEARCHKERNEL_NATIVE_PROBE_VESPA_ROOT
  export SEARCHKERNEL_NATIVE_PROBE_SDK_PREPARED
}

published_vespa_marker_path() {
  printf '%s\n' "$VESPA_ROOT/.searchkernel-vespa-artifact.json"
}

using_published_vespa_toolchain() {
  [[ -f "$(published_vespa_marker_path)" ]]
}

if using_published_vespa_toolchain; then
  SEARCHKERNEL_NATIVE_TOOLCHAIN_ROOT="$(cd "$VESPA_ROOT/../.." && pwd -P)"
  export SEARCHKERNEL_NATIVE_TOOLCHAIN_ROOT
  if [[ -z "${VESPA_DEPS:-}" && -d "$SEARCHKERNEL_NATIVE_TOOLCHAIN_ROOT/vespa-deps" ]]; then
    VESPA_DEPS="$SEARCHKERNEL_NATIVE_TOOLCHAIN_ROOT/vespa-deps"
    export VESPA_DEPS
  fi
fi

resolved_native_base_sdk() {
  if [[ -n "${BASE_SDK:-}" && -d "$BASE_SDK" ]]; then
    printf '%s\n' "$BASE_SDK"
  else
    bash "$REPO_ROOT/tools/resolve-native-sdk.sh" path
  fi
}

expected_vespa_revision() {
  python3 - "$PIN_FILE" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    pin = json.load(stream)
print(pin["commit"])
PY
}

fail_verification() {
  echo "verification error: $*" >&2
  exit 1
}

parse_verifier_args() {
  SKIP_WORKSPACE_QUALITY=0
  while (($#)); do
    case "$1" in
      --skip-workspace-quality)
        SKIP_WORKSPACE_QUALITY=1
        ;;
      *)
        fail_verification "unknown verifier argument: $1"
        ;;
    esac
    shift
  done
}

parse_cumulative_verifier_args() {
  SKIP_WORKSPACE_QUALITY=0
  BASELINE_ALREADY_PROVED=0
  while (($#)); do
    case "$1" in
      --skip-workspace-quality)
        SKIP_WORKSPACE_QUALITY=1
        ;;
      --baseline-already-proved|--skip-cumulative)
        BASELINE_ALREADY_PROVED=1
        ;;
      *)
        fail_verification "unknown cumulative verifier argument: $1"
        ;;
    esac
    shift
  done
}

require_pinned_vespa() {
  local vespa="$VESPA_ROOT"
  local expected actual marker build_env target
  expected="$(expected_vespa_revision)"

  if using_published_vespa_toolchain; then
    marker="$(published_vespa_marker_path)"
    build_env="${SEARCHKERNEL_BUILD_ENV:-host}"
    target="${SK_NATIVE_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
    [[ -n "$target" ]] ||
      fail_verification "could not determine native target for published Vespa provenance"
    [[ -f "$vespa/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/link.txt" &&
       -f "$vespa/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/flags.make" ]] ||
      fail_verification "published Vespa toolchain is missing canonical adapter compile/link metadata: $vespa"

    if ! python3 - "$marker" "$build_env" "$target" "$expected" <<'PY'
import json
import sys

path, build_env, target, commit = sys.argv[1:]
try:
    with open(path, encoding="utf-8") as stream:
        marker = json.load(stream)
except (OSError, json.JSONDecodeError) as exc:
    raise SystemExit(f"invalid published Vespa provenance marker {path}: {exc}") from exc

expected = {
    "build_environment": build_env,
    "native_target": target,
    "vespa_commit": commit,
}
for key, value in expected.items():
    if marker.get(key) != value:
        raise SystemExit(
            f"published Vespa provenance {key} mismatch: "
            f"expected {value!r}, found {marker.get(key)!r}"
        )
generation = marker.get("artifact_generation")
if not isinstance(generation, int) or generation < 1:
    raise SystemExit("published Vespa provenance has no valid artifact_generation")
if marker.get("adapter_static_link_closure") is not True:
    raise SystemExit("published Vespa provenance does not certify adapter static-link closure")

format_version = marker.get("format_version")
legacy_generation_2 = format_version == 1 and generation == 2
if format_version != 2 and not legacy_generation_2:
    raise SystemExit(
        "unsupported published Vespa provenance format: "
        f"format_version={format_version!r}, artifact_generation={generation!r}"
    )
PY
    then
      fail_verification "published Vespa provenance validation failed: $marker"
    fi
    return
  fi

  [[ -d "$vespa/.git" || -f "$vespa/.git" ]] ||
    fail_verification "missing pinned Vespa checkout or published toolchain provenance: $vespa"
  # The verifier is commonly run as root inside the Vespa build container
  # while the checkout is owned by the image's unprivileged build user. Tell
  # Git exactly which checkout is being inspected; this does not relax the
  # pinned-revision or clean-tree checks below.
  actual="$(git -c safe.directory="$vespa" -C "$vespa" rev-parse HEAD 2>/dev/null)" ||
    fail_verification "cannot determine Vespa checkout revision: $vespa"
  [[ "$actual" == "$expected" ]] ||
    fail_verification "expected Vespa $expected, found $actual"
  git -c safe.directory="$vespa" -C "$vespa" diff --quiet -- ||
    fail_verification "tracked Vespa source is dirty: $vespa"
  git -c safe.directory="$vespa" -C "$vespa" diff --cached --quiet -- ||
    fail_verification "staged Vespa source changes found: $vespa"
}

live_oracle_enabled() {
  case "${SEARCHKERNEL_VERIFY_LIVE_ORACLE:-auto}" in
    auto)
      ! using_published_vespa_toolchain
      ;;
    1|true|yes|required)
      if using_published_vespa_toolchain; then
        fail_verification "live upstream oracle requires the configured pinned Vespa source checkout; published-toolchain runners replay the frozen expected corpus"
      fi
      return 0
      ;;
    0|false|no|skip)
      return 1
      ;;
    *)
      fail_verification "invalid SEARCHKERNEL_VERIFY_LIVE_ORACLE='${SEARCHKERNEL_VERIFY_LIVE_ORACLE}'; expected auto|required|skip"
      ;;
  esac
}

announce_frozen_oracle_replay() {
  if [[ "${SEARCHKERNEL_FROZEN_ORACLE_NOTICE_EMITTED:-0}" != 1 ]]; then
    echo "verification notice: published Vespa toolchain detected; replaying frozen expected behavior without rebuilding/rerunning the upstream oracle" >&2
    SEARCHKERNEL_FROZEN_ORACLE_NOTICE_EMITTED=1
    export SEARCHKERNEL_FROZEN_ORACLE_NOTICE_EMITTED
  fi
}

require_sdk_manifest() {
  local manifest="$1"
  local source_header="$REPO_ROOT/core/native/vespa/searchkernel_vespa.h"
  local sdk_root sdk_header

  [[ -f "$manifest" ]] || fail_verification "SDK manifest is missing: $manifest"
  sdk_root="$(cd -- "$(dirname -- "$manifest")" && pwd -P)"
  sdk_header="$sdk_root/include/searchkernel_vespa.h"
  [[ -f "$source_header" ]] || fail_verification "source native ABI header is missing: $source_header"
  [[ -f "$sdk_header" ]] || fail_verification "SDK native ABI header is missing: $sdk_header"
  if find "$sdk_root/lib" -type f \( -name '*.h' -o -name '*.hh' -o -name '*.hpp' \) -print -quit 2>/dev/null | grep -q .; then
    fail_verification "SDK lib directory contains native headers; rebuild the SDK from a clean staging directory"
  fi
  cmp -s "$source_header" "$sdk_header" ||
    fail_verification "SDK native ABI header differs from source header"

  if ! python3 "$REPO_ROOT/tools/check-native-abi-authorities.py" \
      --root "$REPO_ROOT" \
      --sdk-header "$sdk_header" \
      --sdk-manifest "$manifest"; then
    fail_verification "native ABI authority agreement failed"
  fi
}

check_fixture_literals() {
  python3 "$COMMON_DIR/check-fixture-literals.py" "$@"
}

# Rust examples must be executed with `cargo run` from their verifier. Their
# native adapter is a Cargo-produced runtime dependency; launching the example
# path directly bypasses Cargo's runtime search setup after a successful build.
require_native_driver() {
  local binary="$1"
  shift
  [[ -x "$binary" ]] || fail_verification "driver binary is missing or not executable: $binary"
  # Read the complete ldd output. With pipefail enabled, grep -q can make ldd
  # exit on SIGPIPE after finding the library and falsely report failure.
  if ldd "$binary" | grep 'libsearchkernel_vespa' >/dev/null; then
    :
  else
    echo "native driver is not dynamically linked to libsearchkernel_vespa: $binary" >&2
    ldd "$binary" >&2 || true
    exit 1
  fi
  local symbol
  for symbol in "$@"; do
    # Require a strong undefined reference, not a substring or an export from
    # the driver itself. Read the whole stream to preserve pipefail without
    # SIGPIPE, and strip only the ELF version suffix from the exact name.
    nm -D --undefined-only --format=posix "$binary" 2>/dev/null |
      awk -v required="$symbol" '
        { name = $1; sub(/@.*/, "", name) }
        name == required && $2 == "U" { found = 1 }
        END { exit !found }
      ' ||
      fail_verification "native driver does not reference required symbol: $symbol"
  done
}

run_workspace_quality() {
  if [[ "${SKIP_WORKSPACE_QUALITY:-0}" == 1 ]]; then
    echo "workspace quality skipped for nested cumulative verifier"
    return
  fi
  SK_NATIVE_ABI_HEADER="$REPO_ROOT/core/native/vespa/searchkernel_vespa.h" \
    "$REPO_ROOT/tools/check-vespa-bindings.sh"
  cargo fmt --all --check
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  cargo test --workspace --all-targets --all-features
}
