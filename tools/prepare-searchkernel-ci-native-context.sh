#!/usr/bin/env bash
set -euo pipefail

# Prepare immutable Docker build contexts for Search Kernel native CI from a
# verified full SDK plus its configured Vespa toolchain.
#
# This is intentionally independent of the published native-base release/cache
# flow. It exists so the CI image can replace that flow.
#
# Usage:
#   tools/prepare-searchkernel-ci-native-context.sh \
#     <full-sdk> <vespa-root> <output-root> [build-env] [target]
#
# Prints:
#   <output-root>/base
#   <output-root>/toolchain

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"

[[ $# -ge 3 && $# -le 5 ]] || {
    echo "usage: $0 <full-sdk> <vespa-root> <output-root> [build-env] [target]" >&2
    exit 2
}

FULL_SDK="$(cd -- "$1" 2>/dev/null && pwd -P)" || {
    echo "full SDK does not exist: $1" >&2
    exit 1
}
VESPA_ROOT="$(cd -- "$2" 2>/dev/null && pwd -P)" || {
    echo "Vespa root does not exist: $2" >&2
    exit 1
}
OUT_ROOT=$3
BUILD_ENV=${4:-ci}
TARGET=${5:-x86_64-unknown-linux-gnu}

PINNED_VESPA_COMMIT="$(
    python3 - "$ROOT/core/native/vespa/manifest.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    print(json.load(stream)["vespa_commit"])
PY
)"

SDK_MANIFEST="$FULL_SDK/manifest.json"
SDK_FINGERPRINT="$FULL_SDK/native-build-fingerprint-v2.txt"
FLAGS_FILE="$VESPA_ROOT/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/flags.make"
LINK_FILE="$VESPA_ROOT/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/link.txt"

[[ -f "$SDK_MANIFEST" ]] || {
    echo "full SDK manifest is missing: $SDK_MANIFEST" >&2
    exit 1
}
[[ -f "$SDK_FINGERPRINT" ]] || {
    echo "full SDK fingerprint is missing: $SDK_FINGERPRINT" >&2
    exit 1
}
[[ -d "$FULL_SDK/lib" && -s "$FULL_SDK/lib/libvespalib.so" ]] || {
    echo "full SDK does not contain the Vespa runtime closure: $FULL_SDK" >&2
    exit 1
}
[[ -f "$FLAGS_FILE" && -f "$LINK_FILE" ]] || {
    echo "configured Vespa adapter metadata is missing below: $VESPA_ROOT" >&2
    exit 1
}

# Bind the non-Git configured Vespa tree to the full SDK that was produced from
# it. The SDK manifest is bound to its fingerprint digest; that fingerprint
# records the exact configured link.txt/flags.make digest. If the fingerprint
# has an independent successful Git identity for Vespa, require it to equal the
# pinned commit too.
python3 - \
    "$ROOT/core/native/vespa/manifest.json" \
    "$SDK_MANIFEST" \
    "$SDK_FINGERPRINT" \
    "$FLAGS_FILE" \
    "$LINK_FILE" \
    "$TARGET" <<'PY'
import hashlib
import json
import pathlib
import sys

source_manifest_path, sdk_manifest_path, fingerprint_path, flags_path, link_path, target = sys.argv[1:]

with open(source_manifest_path, encoding="utf-8") as stream:
    source_manifest = json.load(stream)
with open(sdk_manifest_path, encoding="utf-8") as stream:
    sdk_manifest = json.load(stream)

pinned_commit = source_manifest.get("vespa_commit")
if sdk_manifest.get("vespa_commit") != pinned_commit:
    raise SystemExit(
        "full SDK Vespa commit mismatch: "
        f"expected {pinned_commit!r}, found {sdk_manifest.get('vespa_commit')!r}"
    )

source_abi = source_manifest.get("native_abi_version")
sdk_abi = sdk_manifest.get("native_abi_version")
if source_abi is not None and sdk_abi != source_abi:
    raise SystemExit(
        f"full SDK native ABI mismatch: expected {source_abi!r}, found {sdk_abi!r}"
    )

text = pathlib.Path(fingerprint_path).read_text(encoding="utf-8")
lines = text.splitlines()
if not lines or lines[0] != "fingerprint-version=3":
    raise SystemExit("full SDK fingerprint is not version 3")
if len(lines) < 2 or not lines[1].startswith("sha256="):
    raise SystemExit("full SDK fingerprint has no sha256 identity")
fingerprint_sha = lines[1].split("=", 1)[1]
if sdk_manifest.get("native_build_fingerprint") != fingerprint_sha:
    raise SystemExit(
        "full SDK manifest/fingerprint mismatch: "
        f"manifest={sdk_manifest.get('native_build_fingerprint')!r}, "
        f"fingerprint={fingerprint_sha!r}"
    )

fields = {}
for line in lines[2:]:
    if "\t" not in line:
        continue
    key, value = line.split("\t", 1)
    fields[key] = value

if fields.get("fingerprint.version") != "3":
    raise SystemExit("full SDK canonical fingerprint version is not 3")
if fields.get("fingerprint.scope") != "full":
    raise SystemExit("full SDK fingerprint scope is not full")
if fields.get("cargo.target") != target:
    raise SystemExit(
        f"full SDK target mismatch: expected {target!r}, found {fields.get('cargo.target')!r}"
    )

def digest_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()

h = hashlib.sha256()
for relative, path in (
    (
        "searchcore/src/tests/proton/documentdb/CMakeFiles/"
        "searchcore_embedded_crud_spike_app.dir/link.txt",
        link_path,
    ),
    (
        "searchcore/src/tests/proton/documentdb/CMakeFiles/"
        "searchcore_embedded_crud_spike_app.dir/flags.make",
        flags_path,
    ),
):
    h.update(relative.encode())
    h.update(b"\0")
    h.update(digest_file(path).encode())
    h.update(b"\n")
current_config = h.hexdigest()
recorded_config = fields.get("vespa.config")
if recorded_config != current_config:
    raise SystemExit(
        "configured Vespa metadata does not match the full SDK fingerprint: "
        f"expected {recorded_config!r}, found {current_config!r}"
    )

# Canonical fingerprint values escape newlines/tabs/backslashes.
def unescape(value):
    out = []
    i = 0
    while i < len(value):
        if value[i] != "\\":
            out.append(value[i])
            i += 1
            continue
        i += 1
        if i >= len(value):
            out.append("\\")
            break
        escaped = value[i]
        out.append({"n": "\n", "r": "\r", "t": "\t", "\\": "\\"}.get(escaped, "\\" + escaped))
        i += 1
    return "".join(out)

vespa_git = unescape(fields.get("vespa.commit", ""))
if vespa_git.startswith("status=0\n"):
    stdout = next(
        (line.removeprefix("stdout=") for line in vespa_git.splitlines() if line.startswith("stdout=")),
        "",
    ).strip()
    if stdout and stdout != pinned_commit:
        raise SystemExit(
            "full SDK fingerprint records a different Vespa Git identity: "
            f"expected {pinned_commit!r}, found {stdout!r}"
        )

print("full SDK / configured Vespa provenance: PASS")
PY

RAW_INCLUDES="$(sed -n 's/^CXX_INCLUDES = //p' "$FLAGS_FILE")"
CONFIGURED_DEPS="$(
    printf '%s\n' "$RAW_INCLUDES" |
        sed -n 's#.*-isystem \([^ ]*\)/include.*#\1#p'
)"

if [[ -n "${VESPA_DEPS:-}" ]]; then
    DEPS=$VESPA_DEPS
elif [[ -d "$ROOT/vespa-deps" ]]; then
    DEPS="$ROOT/vespa-deps"
elif [[ -n "$CONFIGURED_DEPS" && -d "$CONFIGURED_DEPS" ]]; then
    DEPS=$CONFIGURED_DEPS
else
    echo "could not resolve Vespa dependency installation for configured toolchain" >&2
    echo "set VESPA_DEPS explicitly or provide $ROOT/vespa-deps" >&2
    exit 1
fi
DEPS="$(cd -- "$DEPS" && pwd -P)"

[[ -d "$DEPS/include" ]] || {
    echo "Vespa dependency headers are missing: $DEPS/include" >&2
    exit 1
}

BASE="$OUT_ROOT/base"
TOOLCHAIN="$OUT_ROOT/toolchain"
rm -rf "$OUT_ROOT"
mkdir -p \
    "$BASE/lib" \
    "$TOOLCHAIN/upstream/vespa" \
    "$TOOLCHAIN/vespa-deps/include" \
    "$TOOLCHAIN/vespa-deps/lib64"

# The full SDK is already a self-contained runtime closure. Reuse exactly its
# Vespa/runtime library topology, but strip the Search Kernel adapter so each
# checkout must rebuild abi.cc against the immutable base.
cp -a "$FULL_SDK/lib/." "$BASE/lib/"
rm -f \
    "$BASE/lib/libsearchkernel_vespa.so" \
    "$BASE/lib/libsearchkernel_vespa.so.1" \
    "$BASE/lib/libsearchkernel_vespa.so.2"

python3 - "$BASE/manifest.json" "$PINNED_VESPA_COMMIT" <<'PY'
import json
import sys

path, commit = sys.argv[1:]
with open(path, "w", encoding="utf-8") as stream:
    json.dump(
        {"kind": "vespa-base", "vespa_commit": commit},
        stream,
        indent=2,
        sort_keys=True,
    )
    stream.write("\n")
PY

runtime_search_args=()
for runtime_dir in "$DEPS/lib64" "$DEPS/lib"; do
    if [[ -d "$runtime_dir" ]]; then
        runtime_search_args+=(--search-dir "$runtime_dir")
    fi
done
python3 "$ROOT/tools/bundle-native-runtime-closure.py" \
    "$BASE/lib" "${runtime_search_args[@]}"
python3 "$ROOT/tools/check-native-runtime-closure.py" "$BASE/lib"

# Package all source/generated header-like files plus the configured compile and
# link metadata. This path deliberately does not depend on VESPA_ROOT being a
# Git checkout.
(
    cd "$VESPA_ROOT"
    find . -type f \( \
        -name '*.h' -o -name '*.hh' -o -name '*.hpp' -o -name '*.hxx' -o \
        -name '*.ipp' -o -name '*.inl' -o -name '*.inc' -o -name '*.def' -o \
        -name '*.tcc' -o -name 'flags.make' -o -name 'link.txt' \
    \) -print0 |
        tar --null -T - -chf -
) | tar -C "$TOOLCHAIN/upstream/vespa" -xf -

cp -aL "$DEPS/include/." "$TOOLCHAIN/vespa-deps/include/"

# Preserve every production static archive referenced by the harvested Vespa
# link command at its Vespa-root-relative location.
python3 - "$LINK_FILE" "$VESPA_ROOT" "$TOOLCHAIN/upstream/vespa" <<'PY'
import pathlib
import shlex
import shutil
import sys

link_file = pathlib.Path(sys.argv[1]).resolve()
vespa_root = pathlib.Path(sys.argv[2]).resolve()
destination_root = pathlib.Path(sys.argv[3])

tokens = shlex.split(link_file.read_text(encoding="utf-8"))
for token in tokens:
    if not token.endswith(".a"):
        continue
    if pathlib.Path(token).name == "libsearchcore_test.a":
        continue

    if token.startswith("/work/vespa/"):
        source = vespa_root / token.removeprefix("/work/vespa/")
    else:
        source = pathlib.Path(token)
        if not source.is_absolute():
            source = link_file.parent.parent.parent / source
    source = source.resolve()

    try:
        relative = source.relative_to(vespa_root)
    except ValueError as exc:
        raise SystemExit(
            f"adapter link references static archive outside Vespa root: {token}"
        ) from exc

    if not source.is_file():
        raise SystemExit(f"adapter link static archive is missing: {source}")

    destination = destination_root / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
PY

# Normalize producer-local absolute paths to the canonical placeholders already
# handled by build-sdk.sh.
python3 - "$TOOLCHAIN/upstream/vespa" "$VESPA_ROOT" "$DEPS" <<'PY'
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
vespa_root = sys.argv[2]
deps_root = sys.argv[3]

for name in ("flags.make", "link.txt"):
    for path in root.rglob(name):
        text = path.read_text(encoding="utf-8")
        text = text.replace(vespa_root, "/work/vespa")
        text = text.replace(deps_root, "/opt/vespa-deps")
        path.write_text(text, encoding="utf-8")
PY

python3 - \
    "$TOOLCHAIN/upstream/vespa/.searchkernel-vespa-artifact.json" \
    "$BUILD_ENV" "$TARGET" "$PINNED_VESPA_COMMIT" <<'PY'
import json
import sys

path, build_env, target, commit = sys.argv[1:]
with open(path, "w", encoding="utf-8") as stream:
    json.dump(
        {
            "format_version": 2,
            "artifact_generation": 1,
            "adapter_static_link_closure": True,
            "build_environment": build_env,
            "native_target": target,
            "vespa_commit": commit,
            "source": "full-sdk-fingerprint",
        },
        stream,
        indent=2,
        sort_keys=True,
    )
    stream.write("\n")
PY

echo "$BASE"
echo "$TOOLCHAIN"
