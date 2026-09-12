#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
VESPA_ROOT=${VESPA_ROOT:-$ROOT/upstream/vespa}
VESPA_DEPS=${VESPA_DEPS:-/opt/vespa-deps}
VESPA_EXTRA_LIB_DIR=${VESPA_EXTRA_LIB_DIR:-$VESPA_DEPS/../../usr/lib64}
SDK=${SK_NATIVE_SDK:-$ROOT/native/sdk/linux-x86_64}
BUILD=${SK_NATIVE_BUILD_DIR:-$ROOT/target/native-sdk}
CXX=${CXX:-c++}

LINK_FILE=$VESPA_ROOT/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/link.txt
FLAGS_FILE=$VESPA_ROOT/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/flags.make
if [ ! -f "$LINK_FILE" ] || [ ! -f "$FLAGS_FILE" ]; then
    echo "missing configured Vespa build outputs" >&2
    exit 1
fi

mkdir -p "$BUILD" "$SDK/include" "$SDK/lib" "$SDK/share/embedded_crud_cfg"
rm -f "$BUILD/searchkernel_vespa.o" "$SDK/lib/libsearchkernel_vespa.so"

INCLUDES=$(sed -n 's/^CXX_INCLUDES = //p' "$FLAGS_FILE" | sed "s#/work/vespa#$VESPA_ROOT#g; s#/opt/vespa-deps#$VESPA_DEPS#g")
DEFINES=$(sed -n 's/^CXX_DEFINES = //p' "$FLAGS_FILE")
CXX_FLAGS=$(sed -n 's/^CXX_FLAGS = //p' "$FLAGS_FILE")
# The compiler used for the pinned Vespa artifacts is not required to be
# installed at the consumer.  The public SDK only contains the ABI library.
$CXX $DEFINES $INCLUDES $CXX_FLAGS -Wno-inline -std=gnu++23 -fPIC \
    -I"$ROOT/native/vespa" -c "$ROOT/native/vespa/searchkernel_vespa.cpp" \
    -o "$BUILD/searchkernel_vespa.o"

LINK_LINE=$(sed "s#/work/vespa#$VESPA_ROOT#g; s#/opt/vespa-deps#$VESPA_DEPS#g" "$LINK_FILE")
LINK_LINE=$(printf '%s\n' "$LINK_LINE" | sed \
    "s#CMakeFiles/searchcore_embedded_crud_spike_app.dir/embedded_crud_spike.cpp.o#$BUILD/searchkernel_vespa.o#; \
     s#-o searchcore_embedded_crud_spike_app#-shared -Wl,-soname,libsearchkernel_vespa.so -Wl,-rpath,$SDK/lib -o $SDK/lib/libsearchkernel_vespa.so#")
LINK_ARGS=${LINK_LINE#*c++   }
LINK_ARGS=${LINK_ARGS#*c++ }
# Use an old-style RPATH so the SDK's bundled libraries remain discoverable
# for transitive dependencies when the standalone native library is inspected
# or loaded without LD_LIBRARY_PATH.  Keep the SDK relocatable.
LINK_ARGS=$(printf '%s\n' "$LINK_ARGS" | sed \
    "s|-Wl,-rpath,[^ ]*|-Wl,--disable-new-dtags,-rpath,'\$ORIGIN'|g")
LINK_ARGS=$(printf '%s\n' "$LINK_ARGS" | sed "s#-lLLVM-21#$VESPA_EXTRA_LIB_DIR/libLLVM.so.21.1#")
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

cp "$ROOT/native/vespa/searchkernel_vespa.h" "$SDK/include/searchkernel_vespa.h"
cp "$ROOT/native/vespa/manifest.json" "$SDK/manifest.json"
cp "$ROOT/native/vespa/embedded_crud_cfg/"* "$SDK/share/embedded_crud_cfg/"

# Bundle the exact Vespa and third-party shared libraries used by the pinned
# build. Test support libraries are not part of the production ABI package.
# The adapter's private RPATH makes the SDK independent of the build paths.
find "$VESPA_ROOT" -name '*.so*' -exec cp -a -t "$SDK/lib" {} +
find "$VESPA_DEPS/lib64" -maxdepth 1 -name '*.so*' -exec cp -a -t "$SDK/lib" {} +
if [ -f "$VESPA_EXTRA_LIB_DIR/libLLVM.so.21.1" ]; then
    cp -a "$VESPA_EXTRA_LIB_DIR/libLLVM.so.21.1" "$SDK/lib/"
fi
AWS_LIB_DIR=${VESPA_AWS_LIB_DIR:-$VESPA_DEPS/../../usr/local/aws-cli/v2/2.36.43/dist}
if [ -f "$AWS_LIB_DIR/libffi.so.6" ]; then
    cp -a "$AWS_LIB_DIR/libffi.so.6" "$SDK/lib/"
fi
# The upstream build tree also contains unit-test support libraries. Keep
# those out of the distributable SDK even when they share a directory with
# production libraries.
find "$SDK/lib" -maxdepth 1 \( -iname '*test*' -o -iname '*gtest*' \) -delete
if command -v patchelf >/dev/null 2>&1; then
    # Keep the bundled upstream binaries byte-for-byte intact.  The adapter's
    # legacy RPATH is sufficient for transitive dependencies in this private
    # SDK and avoids rewriting unusual upstream ELF layouts.
    patchelf --force-rpath --set-rpath '$ORIGIN' "$SDK/lib/libsearchkernel_vespa.so"
fi

echo "built SDK at $SDK"
