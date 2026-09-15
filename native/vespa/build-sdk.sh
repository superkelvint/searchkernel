#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
VESPA_ROOT=${VESPA_ROOT:-$ROOT/upstream/vespa}
SDK=${SK_NATIVE_SDK:-$ROOT/native/sdk/linux-x86_64}
BUILD=${SK_NATIVE_BUILD_DIR:-$ROOT/target/native-sdk}
CXX=${CXX:-c++}
ENABLE_FAILPOINTS=${SK_NATIVE_ENABLE_FAILPOINTS:-0}
EXTRA_CXX_FLAGS=${SK_NATIVE_EXTRA_CXX_FLAGS:-}
EXTRA_LINK_FLAGS=${SK_NATIVE_EXTRA_LINK_FLAGS:-$EXTRA_CXX_FLAGS}
SANITIZER=${SK_NATIVE_SANITIZER:-none}
FAILPOINT_DEFINES=
if [ "$ENABLE_FAILPOINTS" = 1 ]; then
    FAILPOINT_DEFINES=-DSK_ENABLE_HARDENING_FAILPOINTS
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
VESPA_DEPS=${VESPA_DEPS:-$CONFIGURED_DEPS}
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
VESPA_EXTRA_LIB_DIR=${VESPA_EXTRA_LIB_DIR:-$VESPA_DEPS/../../usr/lib64}

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
$CXX $DEFINES $INCLUDES $CXX_FLAGS $EXTRA_CXX_FLAGS $FAILPOINT_DEFINES \
    -Wno-inline -std=gnu++23 -fPIC \
    -O2 -g \
    -fvisibility=hidden \
    -I"$ROOT/native/vespa" -c "$ROOT/native/vespa/searchkernel_vespa.cpp" \
    -o "$BUILD/searchkernel_vespa.o"

LINK_LINE=$(sed \
    "s|/work/vespa|$VESPA_ROOT|g; s|$CONFIGURED_DEPS/lib64|$DEPS_LIB_DIR|g; s|$CONFIGURED_DEPS/include|$DEPS_INCLUDE_DIR|g; s|$CONFIGURED_DEPS|$VESPA_DEPS|g" \
    "$LINK_FILE")
LINK_LINE=$(printf '%s\n' "$LINK_LINE" | sed \
    "s#CMakeFiles/searchcore_embedded_crud_spike_app.dir/embedded_crud_spike.cpp.o#$BUILD/searchkernel_vespa.o#; \
     s#-o searchcore_embedded_crud_spike_app#-shared -Wl,-soname,libsearchkernel_vespa.so.2 -Wl,-rpath,$SDK/lib -o $SDK/lib/libsearchkernel_vespa.so.2#")
LINK_ARGS=${LINK_LINE#*c++   }
LINK_ARGS=${LINK_ARGS#*c++ }
# The harvested command targets an executable and requests a complete dynamic
# symbol table.  The adapter is a shared library, so do not publish its C++
# implementation symbols.
LINK_ARGS=$(printf '%s\n' "$LINK_ARGS" | sed 's#-rdynamic##g')
# Use an old-style RPATH so the SDK's bundled libraries remain discoverable
# for transitive dependencies when the standalone native library is inspected
# or loaded without LD_LIBRARY_PATH.  Keep the SDK relocatable.
LINK_ARGS=$(printf '%s\n' "$LINK_ARGS" | sed \
    "s|-Wl,-rpath,[^ ]*|-Wl,--disable-new-dtags,-rpath,'\$ORIGIN'|g")
LINK_ARGS=$(printf '%s\n' "$LINK_ARGS" | sed "s#-lLLVM-21#$VESPA_EXTRA_LIB_DIR/libLLVM.so.21.1#")
MAP_FILE=$ROOT/native/vespa/searchkernel_vespa.map
if [ "$ENABLE_FAILPOINTS" = 1 ]; then
    MAP_FILE=$ROOT/native/vespa/searchkernel_vespa_test.map
fi
LINK_ARGS="$LINK_ARGS -Wl,--exclude-libs,ALL -Wl,--version-script=$MAP_FILE"
LINK_ARGS="$LINK_ARGS $EXTRA_LINK_FLAGS"
# The source link command belongs to a Vespa unit-test executable.  The
# adapter-owned replacements below do not need its test-only libraries.
LINK_ARGS=$(printf '%s\n' "$LINK_ARGS" | sed -E \
    -e 's#[^ ]*libsearchcore_test\.a##g' \
    -e 's#[^ ]*libvespa_messagebus-test\.so##g' \
    -e 's#[^ ]*libsearchlib_test\.so##g' \
    -e 's#[^ ]*libsearchlib_searchlib_test_features\.so##g' \
    -e 's#[^ ]*libsearchlib_searchlib_test_memoryindex\.so##g' \
    -e 's#[^ ]*libgtest\.so[^ ]*##g')
cd "$(dirname "$(dirname "$(dirname "$LINK_FILE")")")"
eval "$CXX $LINK_ARGS"

ln -sf libsearchkernel_vespa.so.2 "$SDK/lib/libsearchkernel_vespa.so"

UNEXPECTED_SYMBOLS=$(nm -D --defined-only "$SDK/lib/libsearchkernel_vespa.so.2" |
    awk '$2 ~ /^[A-Z]$/ { name = $3; sub(/@@.*/, "", name); if (name != "SEARCHKERNEL_2" && name !~ /^sk_/) print name }')
if [ -n "$UNEXPECTED_SYMBOLS" ]; then
    echo "unexpected global symbols in Search Kernel ABI library:" >&2
    printf '%s\n' "$UNEXPECTED_SYMBOLS" >&2
    exit 1
fi

cp "$ROOT/native/vespa/searchkernel_vespa.h" "$SDK/include/searchkernel_vespa.h"
if [ "$ENABLE_FAILPOINTS" = 1 ]; then
    cp "$ROOT/native/vespa/searchkernel_vespa_test.h" "$SDK/include/searchkernel_vespa_test.h"
fi
cp "$ROOT/native/vespa/manifest.json" "$SDK/manifest.json"
python3 - "$SDK/manifest.json" "$ENABLE_FAILPOINTS" "$SANITIZER" <<'PY'
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
cp "$ROOT/native/vespa/embedded_crud_cfg/"* "$SDK/share/embedded_crud_cfg/"

# Bundle the exact Vespa and third-party shared libraries used by the pinned
# build. Test support libraries are not part of the production ABI package.
# The adapter's private RPATH makes the SDK independent of the build paths.
find "$VESPA_ROOT" -name '*.so*' -exec cp -a -t "$SDK/lib" {} +
find "$DEPS_LIB_DIR" -maxdepth 1 -name '*.so*' -exec cp -a -t "$SDK/lib" {} +
if [ -f "$VESPA_EXTRA_LIB_DIR/libLLVM.so.21.1" ]; then
    if [ "$(readlink -f "$VESPA_EXTRA_LIB_DIR/libLLVM.so.21.1")" != \
         "$(readlink -f "$SDK/lib/libLLVM.so.21.1" 2>/dev/null || true)" ]; then
        cp -a "$VESPA_EXTRA_LIB_DIR/libLLVM.so.21.1" "$SDK/lib/"
    fi
fi
AWS_LIB_DIR=${VESPA_AWS_LIB_DIR:-$VESPA_DEPS/../../usr/local/aws-cli/v2/2.36.43/dist}
if [ -f "$AWS_LIB_DIR/libffi.so.6" ]; then
    cp -a "$AWS_LIB_DIR/libffi.so.6" "$SDK/lib/"
fi

# Keep the checked-in Rust declarations synchronized with the public native
# header whenever the SDK is rebuilt. This is intentionally a hard failure:
# the header is the ABI source of truth, while ordinary consumers use the
# checked-in bindings and do not need libclang.
SK_NATIVE_SDK="$SDK" "$ROOT/tools/check-vespa-sys-bindings.sh"

# The upstream build tree also contains unit-test support libraries. Keep
# those out of the distributable SDK even when they share a directory with
# production libraries.
find "$SDK/lib" -maxdepth 1 \( -iname '*test*' -o -iname '*gtest*' \) -delete
if command -v patchelf >/dev/null 2>&1; then
    # Keep the bundled upstream binaries byte-for-byte intact.  The adapter's
    # legacy RPATH is sufficient for transitive dependencies in this private
    # SDK and avoids rewriting unusual upstream ELF layouts.
    patchelf --force-rpath --set-rpath '$ORIGIN' "$SDK/lib/libsearchkernel_vespa.so.2"
fi

echo "built SDK at $SDK"
