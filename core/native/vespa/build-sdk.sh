#!/bin/sh
set -eu

# Do not let an application/runtime preload leak into the build tools. In
# particular, preloading libvespalib.so into find, sh, or cp can make those
# unrelated processes segfault while the SDK is being assembled.
if [ -n "${LD_PRELOAD:-}" ]; then
    echo "ignoring LD_PRELOAD during SDK build: $LD_PRELOAD" >&2
    unset LD_PRELOAD
    exec "$0" "$@"
fi

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
VESPA_ROOT=${VESPA_ROOT:-$ROOT/upstream/vespa}
BUILD_ENV=${SEARCHKERNEL_BUILD_ENV:-host}
if [ -n "${CARGO_TARGET_DIR:-}" ]; then
    case "$CARGO_TARGET_DIR" in
        /*) TARGET_ROOT=$CARGO_TARGET_DIR ;;
        *) TARGET_ROOT=$ROOT/$CARGO_TARGET_DIR ;;
    esac
else
    TARGET_ROOT=$ROOT/target/$BUILD_ENV
fi
SDK_PACKAGE=${SK_NATIVE_SDK:-$TARGET_ROOT/native-sdk/linux-x86_64}
case "$SDK_PACKAGE" in
    /*) ;;
    *) SDK_PACKAGE=$ROOT/$SDK_PACKAGE ;;
esac

# Multiple canonical ./dev containers intentionally share one writable Cargo
# target volume for warm builds. Container PIDs are not globally unique, so
# PID-derived staging paths alone cannot protect an SDK package from concurrent
# builders in different containers. Serialize every mutation of one published
# SDK path with a filesystem lock that is shared through that target volume.
SDK_LOCK="${SDK_PACKAGE}.build.lock"
mkdir -p "$(dirname -- "$SDK_LOCK")"
if [ "${SK_NATIVE_SDK_PACKAGE_LOCK_HELD:-}" != "$SDK_PACKAGE" ]; then
    if ! command -v flock >/dev/null 2>&1; then
        echo "native SDK build requires flock for package publication locking" >&2
        exit 1
    fi
    # Re-exec under flock instead of taking a fixed descriptor in this shell.
    # Callers such as resolve-native-sdk.sh already hold their own inherited
    # lock descriptors; replacing one here would silently release that lock.
    SK_NATIVE_SDK_PACKAGE_LOCK_HELD="$SDK_PACKAGE"
    export SK_NATIVE_SDK_PACKAGE_LOCK_HELD
    # Packaged consumer support materializes embedded files with ordinary
    # permissions, so re-enter this script through bash under flock.
    exec flock "$SDK_LOCK" bash "$0" "$@"
fi

# Assemble the package away from the published path. The private staging path
# is allocated immediately before the build so failed prerequisite checks do
# not leave empty staging directories behind.
SDK_STAGING=
SDK=
BUILD=${SK_NATIVE_BUILD_DIR:-$TARGET_ROOT/native-sdk-build}
case "$BUILD" in
    /*) ;;
    *) BUILD=$ROOT/$BUILD ;;
esac
CXX=${CXX:-c++}
PYTHON_BIN=$(bash "$ROOT/tools/resolve-python.sh")
ENABLE_FAILPOINTS=${SK_NATIVE_ENABLE_FAILPOINTS:-0}
ENABLE_COVERAGE=${SK_NATIVE_COVERAGE:-0}
EXTRA_CXX_FLAGS=${SK_NATIVE_EXTRA_CXX_FLAGS:-}
SANITIZER=${SK_NATIVE_SANITIZER:-none}
REGENERATE_BINDINGS=${SK_NATIVE_REGENERATE_BINDINGS:-1}
ADAPTER_ONLY=${SK_NATIVE_ADAPTER_ONLY:-0}
ADAPTER_SONAME=${SK_NATIVE_ADAPTER_SONAME:-libsearchkernel_vespa.so.2}
BASE_ONLY=${SK_NATIVE_BASE_ONLY:-0}
BASE_SDK=${SK_NATIVE_BASE_SDK:-}
if [ "$ADAPTER_ONLY" = 1 ] && [ "$BASE_ONLY" = 1 ]; then
    echo "SK_NATIVE_ADAPTER_ONLY and SK_NATIVE_BASE_ONLY are mutually exclusive" >&2
    exit 1
fi
if [ "$ADAPTER_ONLY" = 1 ]; then
    if [ -z "$BASE_SDK" ] || [ ! -d "$BASE_SDK/lib" ]; then
        echo "SK_NATIVE_ADAPTER_ONLY=1 requires SK_NATIVE_BASE_SDK with a lib directory" >&2
        exit 1
    fi
    BASE_SDK=$(CDPATH= cd -- "$BASE_SDK" && pwd)
fi

# Never let a full/custom SDK rebuild overwrite an immutable published Vespa
# base (or vice versa). Explicit SK_NATIVE_SDK is an override, not permission
# to cross package-kind boundaries.
if [ -f "$SDK_PACKAGE/manifest.json" ]; then
    if grep -Eq '"kind"[[:space:]]*:[[:space:]]*"vespa-base"' "$SDK_PACKAGE/manifest.json"; then
        if [ "$BASE_ONLY" != 1 ]; then
            echo "refusing to overwrite Vespa base '$SDK_PACKAGE' with a Search Kernel adapter/full SDK" >&2
            exit 1
        fi
    elif [ "$BASE_ONLY" = 1 ]; then
        echo "refusing to overwrite full native SDK '$SDK_PACKAGE' with a Vespa base" >&2
        exit 1
    fi
fi
FAILPOINT_DEFINES=
COVERAGE_FLAGS=
COVERAGE_LINK_FLAGS=
OPTIMIZATION_FLAGS=-O2
if [ "$ENABLE_FAILPOINTS" = 1 ]; then
    FAILPOINT_DEFINES=-DSK_ENABLE_HARDENING_FAILPOINTS
fi
if [ "$ENABLE_COVERAGE" = 1 ]; then
    COVERAGE_FLAGS=--coverage
    COVERAGE_LINK_FLAGS=--coverage
    OPTIMIZATION_FLAGS=-O0
fi

LINK_FILE=$VESPA_ROOT/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/link.txt
FLAGS_FILE=$VESPA_ROOT/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/flags.make
if [ ! -f "$LINK_FILE" ] || [ ! -f "$FLAGS_FILE" ]; then
    echo "missing configured Vespa build outputs" >&2
    exit 1
fi

# The configured Vespa build records the dependency root used when it was
# produced. That path is a build-machine detail, not a repository location.
# Reuse it only when it is available; otherwise require the caller to provide
# the dependency installation explicitly rather than feeding stale absolute
# paths to the linker.
RAW_INCLUDES=$(sed -n 's/^CXX_INCLUDES = //p' "$FLAGS_FILE")
CONFIGURED_DEPS=$(printf '%s\n' "$RAW_INCLUDES" |
    sed -n 's#.*-isystem \([^ ]*\)/include.*#\1#p')
VESPA_WORKTREE_ROOT=
case "$VESPA_ROOT" in
    */upstream/vespa)
        VESPA_WORKTREE_ROOT=$(CDPATH= cd -- "$VESPA_ROOT/../.." && pwd)
        ;;
esac
if [ -z "${VESPA_DEPS:-}" ]; then
    if [ -n "$VESPA_WORKTREE_ROOT" ] && [ -d "$VESPA_WORKTREE_ROOT/vespa-deps" ]; then
        VESPA_DEPS=$VESPA_WORKTREE_ROOT/vespa-deps
    elif [ -d "$ROOT/vespa-deps" ]; then
        VESPA_DEPS=$ROOT/vespa-deps
    else
        # A task worktree normally has the source checkout but not the large
        # dependency installation. Prefer a sibling worktree's repository-
        # local vespa-deps when the configured build recorded an unavailable
        # absolute path (for example /opt/vespa-deps in a container).
        WORKTREE_DEPS=$(git -C "$ROOT" worktree list --porcelain 2>/dev/null |
            sed -n 's/^worktree //p' |
            while IFS= read -r WORKTREE; do
                if [ -d "$WORKTREE/vespa-deps" ]; then
                    printf '%s\n' "$WORKTREE/vespa-deps"
                    break
                fi
            done)
        if [ -n "$WORKTREE_DEPS" ]; then
            VESPA_DEPS=$WORKTREE_DEPS
            echo "using Vespa dependencies from worktree: $VESPA_DEPS" >&2
        else
            VESPA_DEPS=$CONFIGURED_DEPS
        fi
    fi
fi
if [ -z "$VESPA_DEPS" ] || [ ! -d "$VESPA_DEPS" ]; then
    echo "Vespa dependencies were not found at the configured build path" >&2
    echo "Set VESPA_DEPS to the dependency installation used by the Vespa build" >&2
    echo "(configured path: ${CONFIGURED_DEPS:-unknown})" >&2
    exit 1
fi

if [ -n "${VESPA_DEPS_LIB_DIR:-}" ]; then
    DEPS_LIB_DIR=$VESPA_DEPS_LIB_DIR
elif [ -d "$VESPA_DEPS/lib64" ]; then
    DEPS_LIB_DIR=$VESPA_DEPS/lib64
elif [ -d "$VESPA_DEPS/lib" ]; then
    DEPS_LIB_DIR=$VESPA_DEPS/lib
else
    echo "Vespa dependency library directory was not found below $VESPA_DEPS" >&2
    echo "Set VESPA_DEPS_LIB_DIR to the directory containing the Vespa libraries" >&2
    exit 1
fi
DEPS_INCLUDE_DIR=${VESPA_DEPS_INCLUDE_DIR:-$VESPA_DEPS/include}
if [ "$ADAPTER_ONLY" = 1 ]; then
    # Adapter-only builds consume the already-validated immutable base. The
    # configured Vespa tree is needed for compile/link metadata only and may no
    # longer contain its original shared-library outputs.
    VESPA_LIBRARY_ROOT=${VESPA_LIBRARY_ROOT:-$BASE_SDK/lib}
else
    VESPA_LIBRARY_ROOT=${VESPA_LIBRARY_ROOT:-$VESPA_ROOT}
fi
if ! find "$VESPA_LIBRARY_ROOT" -name 'libvespa*.so*' -print -quit 2>/dev/null | grep -q .; then
    echo "Vespa shared libraries were not found below $VESPA_LIBRARY_ROOT" >&2
    echo "run './dev native-sdk ensure' from a clone with the pinned Vespa prerequisite or provide VESPA_LIBRARY_ROOT" >&2
    exit 1
fi
has_llvm_runtime() {
    [ -f "$1/libLLVM.so.21.1" ] || [ -f "$1/libLLVM-21.so" ]
}
if [ -z "${VESPA_EXTRA_LIB_DIR:-}" ]; then
    if [ "$ADAPTER_ONLY" = 1 ] && has_llvm_runtime "$BASE_SDK/lib"; then
        VESPA_EXTRA_LIB_DIR=$BASE_SDK/lib
    elif [ "$BASE_ONLY" = 1 ] && has_llvm_runtime "$DEPS_LIB_DIR"; then
        VESPA_EXTRA_LIB_DIR=$DEPS_LIB_DIR
    elif [ "$BASE_ONLY" = 1 ] && has_llvm_runtime "$VESPA_DEPS/../../usr/lib64"; then
        VESPA_EXTRA_LIB_DIR=$VESPA_DEPS/../../usr/lib64
    elif [ "$BASE_ONLY" != 1 ] && has_llvm_runtime "$SDK_PACKAGE/lib"; then
        VESPA_EXTRA_LIB_DIR=$SDK_PACKAGE/lib
    elif has_llvm_runtime "$VESPA_DEPS/../../usr/lib64"; then
        VESPA_EXTRA_LIB_DIR=$VESPA_DEPS/../../usr/lib64
    else
        VESPA_EXTRA_LIB_DIR=$DEPS_LIB_DIR
    fi
fi
if [ -f "$VESPA_EXTRA_LIB_DIR/libLLVM.so.21.1" ]; then
    VESPA_EXTRA_LLVM=$VESPA_EXTRA_LIB_DIR/libLLVM.so.21.1
elif [ -f "$VESPA_EXTRA_LIB_DIR/libLLVM-21.so" ]; then
    VESPA_EXTRA_LLVM=$VESPA_EXTRA_LIB_DIR/libLLVM-21.so
else
    echo "LLVM 21 runtime was not found for the Vespa SDK link" >&2
    echo "looked in: $VESPA_EXTRA_LIB_DIR" >&2
    echo "expected libLLVM-21.so or libLLVM.so.21.1" >&2
    exit 1
fi
EXTRA_LINK_FLAGS=${SK_NATIVE_EXTRA_LINK_FLAGS:-$EXTRA_CXX_FLAGS}
if [ -f "$VESPA_EXTRA_LIB_DIR/libxxhash.so" ]; then
    EXTRA_LINK_FLAGS="$EXTRA_LINK_FLAGS -L$VESPA_EXTRA_LIB_DIR"
fi

if [ -n "${SK_NATIVE_STAGING_SDK:-}" ]; then
    SDK_STAGING=$SK_NATIVE_STAGING_SDK
    if [ -e "$SDK_STAGING" ]; then
        echo "SDK staging directory already exists: $SDK_STAGING" >&2
        exit 1
    fi
    mkdir -p "$SDK_STAGING"
else
    # $$ is only unique within one PID namespace. Canonical ./dev invocations
    # run in separate containers while sharing /cargo-target, so use mktemp's
    # random suffix to make the staging path unique across containers as well.
    SDK_STAGING=$(mktemp -d "${SDK_PACKAGE}.staging.XXXXXX")
fi
SDK=$SDK_STAGING

cleanup_staging() {
    if [ -d "$SDK_STAGING" ]; then
        rm -rf "$SDK_STAGING"
    fi
}
trap cleanup_staging EXIT

mkdir -p "$BUILD" "$SDK/include" "$SDK/lib" "$SDK/share/embedded_crud_cfg"
rm -f "$BUILD/searchkernel_vespa.o" \
    "$SDK/lib/libsearchkernel_vespa.so" "$SDK/lib/libsearchkernel_vespa.so.1" \
    "$SDK/lib/libsearchkernel_vespa.so.2"

INCLUDES=$(printf '%s\n' "$RAW_INCLUDES" | sed \
    "s|/work/vespa|$VESPA_ROOT|g; s|$CONFIGURED_DEPS/include|$DEPS_INCLUDE_DIR|g; s|$CONFIGURED_DEPS|$VESPA_DEPS|g")
DEFINES=$(sed -n 's/^CXX_DEFINES = //p' "$FLAGS_FILE")
CXX_FLAGS=$(sed -n 's/^CXX_FLAGS = //p' "$FLAGS_FILE")
# The compiler used for the pinned Vespa artifacts is not required to be
# installed at the consumer.  The public SDK only contains the ABI library.
# Keep DWARF in this artifact deliberately: the ABI release check uses
# libabigail to compare the types reachable from the exported C functions.
# These explicit final flags make that property independent of the harvested
# test target's optimization/debug configuration.
if [ "$BASE_ONLY" != 1 ]; then
$CXX $DEFINES $INCLUDES $CXX_FLAGS $EXTRA_CXX_FLAGS $FAILPOINT_DEFINES $COVERAGE_FLAGS \
    -Wno-inline -std=gnu++23 -fPIC \
    $OPTIMIZATION_FLAGS -g \
    -fvisibility=hidden \
    -I"$ROOT/core/native/vespa" -c "$ROOT/core/native/vespa/abi.cc" \
    -o "$BUILD/searchkernel_vespa.o"

LINK_LINE=$(sed \
    "s|/work/vespa|$VESPA_ROOT|g; s|$CONFIGURED_DEPS/lib64|$DEPS_LIB_DIR|g; s|$CONFIGURED_DEPS/include|$DEPS_INCLUDE_DIR|g; s|$CONFIGURED_DEPS|$VESPA_DEPS|g" \
    "$LINK_FILE")
LINK_LINE=$(printf '%s\n' "$LINK_LINE" | sed \
    "s#CMakeFiles/searchcore_embedded_crud_spike_app.dir/embedded_crud_spike.cpp.o#$BUILD/searchkernel_vespa.o#; \
     s#-o searchcore_embedded_crud_spike_app#-shared -Wl,-soname,'$ADAPTER_SONAME' -Wl,-rpath,$SDK/lib -o $SDK/lib/libsearchkernel_vespa.so.2#")
LINK_ARGS=${LINK_LINE#*c++   }
LINK_ARGS=${LINK_ARGS#*c++ }
# The harvested command targets an executable and requests a complete dynamic
# symbol table.  The adapter is a shared library, so do not publish its C++
# implementation symbols.
LINK_ARGS=$(printf '%s\n' "$LINK_ARGS" | sed 's#-rdynamic##g')
# Use an old-style RPATH so the SDK's bundled libraries remain discoverable
# for transitive dependencies when the standalone native library is inspected
# or loaded without LD_LIBRARY_PATH.  Keep the SDK relocatable.
if [ "$ADAPTER_ONLY" = 1 ]; then
    ADAPTER_RPATH=$BASE_SDK/lib
else
    ADAPTER_RPATH='$ORIGIN'
fi
LINK_ARGS=$(printf '%s\n' "$LINK_ARGS" | sed \
    "s|-Wl,-rpath,[^ ]*|-Wl,--disable-new-dtags,-rpath,'$ADAPTER_RPATH'|g")
LINK_ARGS=$(printf '%s\n' "$LINK_ARGS" | sed "s#-lLLVM-21#$VESPA_EXTRA_LLVM#")
MAP_FILE=$ROOT/core/native/vespa/searchkernel_vespa.map
if [ "$ENABLE_FAILPOINTS" = 1 ]; then
    MAP_FILE=$ROOT/core/native/vespa/searchkernel_vespa_test.map
fi
if [ "$ADAPTER_ONLY" = 1 ]; then
    # Keep the local adapter export surface identical without assigning a named
    # ELF symbol version. glibc cannot reconcile a version-need filename that
    # contains $ORIGIN after expanding that token in DT_NEEDED.
    MAP_FILE=$BUILD/searchkernel_vespa_local.map
    printf '%s\n' '{ global: sk_*; local: *; };' > "$MAP_FILE"
fi
LINK_ARGS="$LINK_ARGS -Wl,--exclude-libs,ALL -Wl,--version-script=$MAP_FILE"
LINK_ARGS="$LINK_ARGS $EXTRA_LINK_FLAGS $COVERAGE_LINK_FLAGS"
if [ "$ENABLE_FAILPOINTS" = 1 ]; then
    # Hardening entry points are intentionally referenced only by the M11
    # driver and coverage probes. Keep them alive when the Vespa link command
    # enables section garbage collection, and explicitly place them in the
    # dynamic symbol table instead of relying on linker heuristics.
    TEST_SYMBOLS=$(sed -nE \
        's/^SK_API ([A-Za-z0-9_]+) (sk_test_[a-zA-Z0-9_]+)\(.*/\2/p' \
        "$ROOT/core/native/vespa/searchkernel_vespa_test.h")
    for symbol in $TEST_SYMBOLS; do
        LINK_ARGS="$LINK_ARGS -Wl,--undefined=$symbol -Wl,--export-dynamic-symbol=$symbol"
    done
fi
# The source link command belongs to a Vespa unit-test executable.  The
# adapter-owned replacements below do not need its test-only libraries.
LINK_ARGS=$(printf '%s\n' "$LINK_ARGS" | sed -E \
    -e 's#[^ ]*libsearchcore_test\.a##g' \
    -e 's#[^ ]*libvespa_messagebus-test\.so##g' \
    -e 's#[^ ]*libsearchlib_test\.so##g' \
    -e 's#[^ ]*libsearchlib_searchlib_test_features\.so##g' \
    -e 's#[^ ]*libsearchlib_searchlib_test_memoryindex\.so##g' \
    -e 's#[^ ]*libgtest\.so[^ ]*##g')

if [ "$ADAPTER_ONLY" = 1 ]; then
    # The configured Vespa tree supplies compile flags and link topology only.
    # Every shared-library input for the adapter must come from the immutable
    # base object it will run against, never from the mutable build tree.
    for BASE_LIBRARY in "$BASE_SDK"/lib/*.so*; do
        [ -f "$BASE_LIBRARY" ] || continue
        LIB_NAME=$(basename "$BASE_LIBRARY")
        LINK_ARGS=$(printf '%s\n' "$LINK_ARGS" | sed "s#[^ ]*/$LIB_NAME#$BASE_LIBRARY#g")
    done
    LINK_ARGS="-L$BASE_SDK/lib $LINK_ARGS"
fi

cd "$(dirname "$(dirname "$(dirname "$LINK_FILE")")")"
eval "$CXX $LINK_ARGS"

# rust-lld does not resolve a -lNAME request through a symlink named
# libNAME.so in this SDK layout. Keep the unversioned linker name as a real
# file while retaining the versioned SONAME artifact for runtime loading.
# A hardlink satisfies that linker constraint without storing a second 100+ MB
# copy of the exact same adapter payload.
rm -f "$SDK/lib/libsearchkernel_vespa.so"
ln "$SDK/lib/libsearchkernel_vespa.so.2" "$SDK/lib/libsearchkernel_vespa.so"

UNEXPECTED_SYMBOLS=$(nm -D --defined-only "$SDK/lib/libsearchkernel_vespa.so.2" |
    awk '$2 ~ /^[A-Z]$/ { name = $3; sub(/@@.*/, "", name); if (name != "SEARCHKERNEL_2" && name !~ /^sk_/) print name }')
if [ -n "$UNEXPECTED_SYMBOLS" ]; then
    echo "unexpected global symbols in Search Kernel ABI library:" >&2
    printf '%s\n' "$UNEXPECTED_SYMBOLS" >&2
    exit 1
fi
fi

# Development builds treat the pinned Vespa/runtime closure as an immutable
# prerequisite. Rebuild only Search Kernel's adapter into Cargo OUT_DIR and
# point its RPATH at that shared base. Do not copy or mutate the Vespa base.
if [ "$ADAPTER_ONLY" = 1 ]; then
    if [ "${SK_NATIVE_SKIP_ABI_AUTHORITIES:-0}" != 1 ]; then
        $PYTHON_BIN "$ROOT/tools/check-native-abi-authorities.py" --root "$ROOT"
    fi
    if command -v patchelf >/dev/null 2>&1; then
        patchelf --force-rpath --set-rpath "$BASE_SDK/lib" "$SDK/lib/libsearchkernel_vespa.so.2"
        rm -f "$SDK/lib/libsearchkernel_vespa.so"
        ln "$SDK/lib/libsearchkernel_vespa.so.2" "$SDK/lib/libsearchkernel_vespa.so"
    fi
    cp "$ROOT/core/native/vespa/searchkernel_vespa.h" "$SDK/include/searchkernel_vespa.h"
    if [ "$ENABLE_FAILPOINTS" = 1 ]; then
        cp "$ROOT/core/native/vespa/searchkernel_vespa_test.h" "$SDK/include/searchkernel_vespa_test.h"
    fi
    cp "$ROOT/core/native/vespa/manifest.json" "$SDK/manifest.json"
    cp "$ROOT/core/native/vespa/embedded_crud_cfg/"* "$SDK/share/embedded_crud_cfg/"
    $PYTHON_BIN - "$SDK/manifest.json" "$ENABLE_FAILPOINTS" "$SANITIZER" <<'PY'
import json
import sys

path, failpoints, sanitizer = sys.argv[1:]
with open(path, encoding="utf-8") as stream:
    manifest = json.load(stream)
manifest["hardening_failpoints"] = failpoints == "1"
manifest["sanitizer"] = sanitizer
with open(path, "w", encoding="utf-8") as stream:
    json.dump(manifest, stream, indent=2, sort_keys=True)
    stream.write("\n")
PY

    PYTHON_BIN=$(bash "$ROOT/tools/resolve-python.sh")
    RUSTC_BIN=$(bash "$ROOT/tools/resolve-rust.sh" rustc)
    NATIVE_TARGET=${SK_NATIVE_TARGET:-$("$RUSTC_BIN" -vV | sed -n 's/^host: //p')}
    if [ -z "$NATIVE_TARGET" ]; then
        echo "could not determine the Rust host target; set SK_NATIVE_TARGET" >&2
        exit 1
    fi
    RUSTC="$RUSTC_BIN" "$PYTHON_BIN" "$ROOT/tools/native-build-fingerprint.py" \
        --root "$ROOT" \
        --vespa-root "$VESPA_ROOT" \
        --base-sdk "$BASE_SDK" \
        --target "$NATIVE_TARGET" \
        --scope full \
        --output "$SDK/native-build-fingerprint-v2.txt"

    SDK_BACKUP=$(mktemp -d "${SDK_PACKAGE}.old.XXXXXX")
    rmdir "$SDK_BACKUP"
    if [ -e "$SDK_BACKUP" ]; then
        echo "adapter backup path already exists: $SDK_BACKUP" >&2
        exit 1
    fi
    if [ -e "$SDK_PACKAGE" ]; then
        mv "$SDK_PACKAGE" "$SDK_BACKUP"
    fi
    mv "$SDK" "$SDK_PACKAGE"
    if [ -e "$SDK_BACKUP" ]; then
        rm -rf "$SDK_BACKUP"
    fi
    trap - EXIT
    echo "built Search Kernel adapter at $SDK_PACKAGE"
    exit 0
fi

if [ "$BASE_ONLY" != 1 ]; then
    cp "$ROOT/core/native/vespa/searchkernel_vespa.h" "$SDK/include/searchkernel_vespa.h"
    if [ "$ENABLE_FAILPOINTS" = 1 ]; then
        cp "$ROOT/core/native/vespa/searchkernel_vespa_test.h" "$SDK/include/searchkernel_vespa_test.h"
    fi
    cp "$ROOT/core/native/vespa/manifest.json" "$SDK/manifest.json"

    NATIVE_TARGET=${SK_NATIVE_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
    if [ -z "$NATIVE_TARGET" ]; then
        echo "could not determine the Rust host target; set SK_NATIVE_TARGET" >&2
        exit 1
    fi
    $PYTHON_BIN "$ROOT/tools/native-build-fingerprint.py" \
        --root "$ROOT" \
        --vespa-root "$VESPA_ROOT" \
        --target "$NATIVE_TARGET" \
        --scope full \
        --output "$SDK/native-build-fingerprint-v2.txt"
    $PYTHON_BIN - "$SDK/manifest.json" "$ROOT/core/native/vespa/searchkernel_vespa.h" "$ENABLE_FAILPOINTS" "$SANITIZER" <<'PY'
import json
import re
import sys

path, source_header, failpoints, sanitizer = sys.argv[1:]
with open(path, encoding="utf-8") as stream:
    manifest = json.load(stream)
with open(source_header, encoding="utf-8") as stream:
    header = stream.read()
matches = re.findall(r"^#define SK_NATIVE_ABI_VERSION ([0-9]+)U$", header, re.MULTILINE)
if len(matches) != 1:
    raise SystemExit(
        f"expected exactly one SK_NATIVE_ABI_VERSION in {source_header}, found {len(matches)}"
    )
manifest["native_abi_version"] = int(matches[0])
manifest["hardening_failpoints"] = failpoints == "1"
manifest["sanitizer"] = sanitizer
with open(path.replace("manifest.json", "native-build-fingerprint-v2.txt"), encoding="utf-8") as stream:
    manifest["native_build_fingerprint"] = next(
        line.split("=", 1)[1].strip()
        for line in stream
        if line.startswith("sha256=")
    )
with open(path, "w", encoding="utf-8") as stream:
    json.dump(manifest, stream, indent=2, sort_keys=True)
    stream.write("\n")
PY
    cp "$ROOT/core/native/vespa/embedded_crud_cfg/"* "$SDK/share/embedded_crud_cfg/"
else
    $PYTHON_BIN - "$ROOT/core/native/vespa/manifest.json" "$SDK/manifest.json" <<'PY'
import json
import sys

source_path, output_path = sys.argv[1:]
with open(source_path, encoding="utf-8") as stream:
    source = json.load(stream)
with open(output_path, "w", encoding="utf-8") as stream:
    json.dump(
        {"kind": "vespa-base", "vespa_commit": source["vespa_commit"]},
        stream,
        indent=2,
        sort_keys=True,
    )
    stream.write("\n")
PY
fi

# Bundle the exact Vespa and third-party shared libraries used by the pinned
# build. Test support libraries are not part of the production ABI package.
# Dependency installs often expose unversioned linker names as absolute
# symlinks whose ELF DT_SONAME is a different runtime name (for example,
# libxxhash.so -> .../libxxhash.so.0.8.2 with SONAME libxxhash.so.0).
# Copy each resolved payload once, then recreate linker and SONAME aliases as
# relative links inside SDK/lib. This keeps the package self-contained without
# duplicating large shared objects for every alias.
if [ "$BASE_ONLY" = 1 ]; then
    LINKER_ALIASES=()
    if [ -f "$VESPA_EXTRA_LLVM" ]; then
        LINKER_ALIASES+=(--linker-alias "LLVM-21=$VESPA_EXTRA_LLVM")
    fi
    if [ -n "$CONFIGURED_DEPS" ]; then
        LINKER_ALIASES+=(--producer-root "$CONFIGURED_DEPS=$VESPA_DEPS")
    fi
    LINKER_ALIASES+=(--producer-root "/work/vespa=$VESPA_ROOT")
    LINKER_ALIASES+=(--source-root "vespa-build=$VESPA_ROOT")
    LINKER_ALIASES+=(--source-root "vespa-deps=$VESPA_DEPS")
    LINKER_ALIASES+=(--source-root "vespa-extra=$VESPA_EXTRA_LIB_DIR")
    LINKER_ALIASES+=(--source-root "system=/usr")
    "$PYTHON_BIN" "$ROOT/tools/copy-native-base-link-shared-libraries.py" \
        "$LINK_FILE" "$SDK/lib" --cxx "$CXX" "${LINKER_ALIASES[@]}"
elif [ "$VESPA_LIBRARY_ROOT" = "$VESPA_ROOT" ]; then
    find "$VESPA_ROOT" -name '*.so*' \
        -exec sh "$ROOT/tools/copy-shared-library-into-sdk.sh" "$SDK/lib" {} +
else
    # Migration/bootstrap path for clones that already have a self-contained
    # full SDK from before the immutable-base split. The resolver verifies that
    # SDK's Vespa commit/config identity before selecting its lib directory.
    find "$VESPA_LIBRARY_ROOT" -maxdepth 1 -name 'libvespa*.so*' \
        -exec sh "$ROOT/tools/copy-shared-library-into-sdk.sh" "$SDK/lib" {} +
fi
# Do not mirror the dependency installation by filename for a Vespa base.
# Runtime DSOs are added below by the ELF resolver only when a pinned Vespa DSO
# actually loads them; this avoids shadowing a compatible producer choice with
# an unrelated same-name file from lib64. The legacy full development SDK
# keeps its existing convenience copy behavior.
if [ "$BASE_ONLY" != 1 ]; then
    find "$DEPS_LIB_DIR" -maxdepth 1 -name '*.so*' \
        ! -name 'libsearchkernel_vespa.so*' ! -name 'libffi.so.6' \
        -exec sh "$ROOT/tools/copy-shared-library-into-sdk.sh" "$SDK/lib" {} +
fi
if [ "$BASE_ONLY" != 1 ]; then
    # Full development SDKs retain their historical convenience behavior.
    # Native-base construction leaves every non-system runtime dependency,
    # including libffi, to the generic DT_NEEDED resolver below.
    if [ -n "${VESPA_AWS_LIB_DIR:-}" ]; then
        AWS_LIB_DIR=$VESPA_AWS_LIB_DIR
    elif [ -f "$VESPA_DEPS/../../usr/local/aws-cli/v2/2.36.43/dist/libffi.so.6" ]; then
        AWS_LIB_DIR=$VESPA_DEPS/../../usr/local/aws-cli/v2/2.36.43/dist
    elif [ -f "$DEPS_LIB_DIR/libffi.so.6" ]; then
        AWS_LIB_DIR=$DEPS_LIB_DIR
    elif [ -f "$SDK_PACKAGE/lib/libffi.so.6" ]; then
        AWS_LIB_DIR=$SDK_PACKAGE/lib
    else
        AWS_LIB_DIR=$ROOT/target/native-sdk-package/lib
    fi
    if [ -f "$AWS_LIB_DIR/libffi.so.6" ]; then
        if [ "$(readlink -f "$AWS_LIB_DIR/libffi.so.6")" != \
             "$(readlink -f "$SDK/lib/libffi.so.6" 2>/dev/null || true)" ]; then
            sh "$ROOT/tools/copy-shared-library-into-sdk.sh" "$SDK/lib" "$AWS_LIB_DIR/libffi.so.6"
        fi
    elif [ -f "$SDK_PACKAGE/lib/libffi.so.6" ]; then
        sh "$ROOT/tools/copy-shared-library-into-sdk.sh" "$SDK/lib" "$SDK_PACKAGE/lib/libffi.so.6"
    elif [ -f "$DEPS_LIB_DIR/libffi.so.6" ]; then
        sh "$ROOT/tools/copy-shared-library-into-sdk.sh" "$SDK/lib" "$DEPS_LIB_DIR/libffi.so.6"
    else
        echo "libffi.so.6 required by the bundled LLVM runtime was not found" >&2
        exit 1
    fi
fi

# A base object must be self-contained before it is admitted to the immutable
# native cache. Runtime sources are restricted to explicitly named producer
# roots so transitive symlinks cannot pull in another local SDK.
if [ "$BASE_ONLY" = 1 ]; then
    RUNTIME_SEARCH_ARGS=(--search-dir "$DEPS_LIB_DIR" --search-dir "$VESPA_EXTRA_LIB_DIR")
    "$PYTHON_BIN" "$ROOT/tools/bundle-native-runtime-closure.py" \
        "$SDK/lib" \
        "${RUNTIME_SEARCH_ARGS[@]}" \
        --source-root "vespa-build=$VESPA_ROOT" \
        --source-root "vespa-deps=$VESPA_DEPS" \
        --source-root "vespa-extra=$VESPA_EXTRA_LIB_DIR" \
        --source-root "system=/usr" \
        --source-root "system-lib=/lib" \
        --source-root "system-lib64=/lib64"
fi


# Shared-library aliases are allowed only when they are simple relative names
# resolving to regular files inside this SDK. Never publish producer absolute
# links, directory traversal, broken links, or links back into the host.
SDK_LIB_REAL=$(CDPATH= cd -- "$SDK/lib" && pwd -P)
find "$SDK/lib" -maxdepth 1 -type l -print |
while IFS= read -r LINK; do
    TARGET=$(readlink "$LINK" 2>/dev/null || true)
    case "$TARGET" in
        ''|/*|*/*)
            echo "refusing unsafe native SDK shared-library symlink: $LINK -> ${TARGET:-unreadable}" >&2
            exit 1
            ;;
    esac
    RESOLVED=$(readlink -f "$LINK" 2>/dev/null || true)
    case "$RESOLVED" in
        "$SDK_LIB_REAL"/*) ;;
        *)
            echo "refusing native SDK shared-library symlink escaping SDK/lib: $LINK -> ${RESOLVED:-unresolved}" >&2
            exit 1
            ;;
    esac
    if [ ! -f "$RESOLVED" ]; then
        echo "refusing broken native SDK shared-library symlink: $LINK -> $TARGET" >&2
        exit 1
    fi
done

# Keep the checked-in Rust declarations synchronized with the public native
# header whenever the SDK is rebuilt. This is intentionally a hard failure:
# the header is the ABI source of truth, while ordinary consumers use the
# checked-in bindings and do not need libclang.
if [ "$BASE_ONLY" != 1 ] && [ "$REGENERATE_BINDINGS" = 1 ]; then
    if [ "${SK_NATIVE_SKIP_BINDINGS_CHECK:-0}" != 1 ]; then
        SK_NATIVE_SDK="$SDK" SK_NATIVE_BINDINGS_REGENERATE=1 \
            "$ROOT/tools/check-vespa-bindings.sh"
    fi
elif [ "$BASE_ONLY" != 1 ] && [ "${SK_NATIVE_SKIP_BINDINGS_CHECK:-0}" != 1 ]; then
    SK_NATIVE_SDK="$SDK" "$ROOT/tools/check-vespa-bindings.sh"
fi
if [ "$BASE_ONLY" != 1 ]; then
    $PYTHON_BIN "$ROOT/tools/check-native-abi-authorities.py" --root "$ROOT"
fi

# The upstream build tree also contains unit-test support libraries. Keep
# those out of the distributable SDK even when they share a directory with
# production libraries.
find "$SDK/lib" -maxdepth 1 \( -iname '*test*' -o -iname '*gtest*' \) -delete
if [ "$BASE_ONLY" != 1 ] && command -v patchelf >/dev/null 2>&1; then
    # Keep the bundled upstream binaries byte-for-byte intact. The adapter's
    # legacy RPATH is sufficient for transitive dependencies in a full package.
    patchelf --force-rpath --set-rpath '$ORIGIN' "$SDK/lib/libsearchkernel_vespa.so.2"
fi

# Headers belong in include/, never beside runtime libraries. Rejecting them
# here catches contaminated source/build trees before publication.
if find "$SDK/lib" -type f \( -name '*.h' -o -name '*.hh' -o -name '*.hpp' \) -print -quit | grep -q .; then
    echo "refusing to publish native headers under SDK/lib" >&2
    exit 1
fi
if [ "$BASE_ONLY" != 1 ] && [ "$ENABLE_FAILPOINTS" = 1 ]; then
    "$ROOT/tools/check-native-sdk-hardening.sh" "$SDK"
fi

# Publish only after every build, ABI, binding, and package check succeeded.
# A uniquely named backup keeps the replacement recoverable until the new
# directory is in place; the old generated package is then removed explicitly.
SDK_BACKUP=$(mktemp -d "${SDK_PACKAGE}.old.XXXXXX")
rmdir "$SDK_BACKUP"
if [ -e "$SDK_BACKUP" ]; then
    echo "SDK backup path already exists: $SDK_BACKUP" >&2
    exit 1
fi
if [ -e "$SDK_PACKAGE" ]; then
    mv "$SDK_PACKAGE" "$SDK_BACKUP"
fi
mv "$SDK" "$SDK_PACKAGE"
if [ -e "$SDK_BACKUP" ]; then
    rm -rf "$SDK_BACKUP"
fi
trap - EXIT

echo "built SDK at $SDK_PACKAGE"
