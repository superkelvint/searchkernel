#!/usr/bin/env bash
set -euo pipefail

# Cargo build-script linker arguments are package-local: an rpath emitted by
# searchkernel-vespa-bindings does not propagate through its rlib into a
# downstream executable. This wrapper runs at the final Rust link step instead.
# If the link consumes the Search Kernel native adapter, derive the exact
# Cargo-published adapter directory from the linker's own -L search path and
# embed an output-relative ELF RUNPATH. No caller-side LD_LIBRARY_PATH is needed.

REAL_LINKER=${SEARCHKERNEL_REAL_LINKER:-cc}
ARGS=("$@")
NATIVE_LINK=0
NATIVE_LIB_DIR=
OUTPUT=
EXPECT_LIB_DIR=0
EXPECT_LIB_NAME=0
EXPECT_OUTPUT=0

consider_lib_dir() {
    local candidate=$1
    candidate=${candidate#native=}
    if [[ -z "$NATIVE_LIB_DIR" && -f "$candidate/libsearchkernel_vespa.so.2" ]]; then
        NATIVE_LIB_DIR=$candidate
    fi
}

for arg in "$@"; do
    if (( EXPECT_LIB_DIR )); then
        consider_lib_dir "$arg"
        EXPECT_LIB_DIR=0
        continue
    fi
    if (( EXPECT_LIB_NAME )); then
        if [[ "$arg" == "searchkernel_vespa" ||
              "$arg" == ":libsearchkernel_vespa.so" ||
              "$arg" == ":libsearchkernel_vespa.so.2" ]]; then
            NATIVE_LINK=1
        fi
        EXPECT_LIB_NAME=0
        continue
    fi
    if (( EXPECT_OUTPUT )); then
        OUTPUT=$arg
        EXPECT_OUTPUT=0
        continue
    fi

    case "$arg" in
        -lsearchkernel_vespa|-l:libsearchkernel_vespa.so|-l:libsearchkernel_vespa.so.2)
            NATIVE_LINK=1
            ;;
        -l)
            EXPECT_LIB_NAME=1
            ;;
        -L)
            EXPECT_LIB_DIR=1
            ;;
        -L*)
            consider_lib_dir "${arg#-L}"
            ;;
        -o)
            EXPECT_OUTPUT=1
            ;;
        -o*)
            OUTPUT=${arg#-o}
            ;;
    esac
done

if (( NATIVE_LINK )); then
    if [[ -z "$NATIVE_LIB_DIR" ]]; then
        echo "Search Kernel native linker could not resolve libsearchkernel_vespa.so.2 from linker search paths" >&2
        exit 70
    fi
    if [[ -z "$OUTPUT" ]]; then
        echo "Search Kernel native linker could not determine final linker output path" >&2
        exit 70
    fi

    OUTPUT_DIR=$(realpath -m -- "$(dirname -- "$OUTPUT")")
    NATIVE_LIB_DIR=$(realpath -- "$NATIVE_LIB_DIR")
    RELATIVE_LIB_DIR=$(realpath --relative-to="$OUTPUT_DIR" -- "$NATIVE_LIB_DIR")
    if [[ "$RELATIVE_LIB_DIR" == "." ]]; then
        RUNPATH='$ORIGIN'
    else
        RUNPATH="\$ORIGIN/$RELATIVE_LIB_DIR"
    fi

    # Force modern DT_RUNPATH semantics and mark $ORIGIN expansion explicitly.
    # -Xlinker avoids comma-splitting paths passed through the compiler driver.
    ARGS+=(
        "-Wl,--enable-new-dtags"
        "-Wl,-z,origin"
        "-Xlinker" "-rpath"
        "-Xlinker" "$RUNPATH"
    )
fi

exec "$REAL_LINKER" "${ARGS[@]}"
