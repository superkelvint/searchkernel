#!/bin/sh
set -eu

if [ "$#" -lt 2 ]; then
    echo "usage: $0 DEST_LIB_DIR SOURCE..." >&2
    exit 2
fi

DEST_DIR=$1
shift
mkdir -p "$DEST_DIR"

validate_name() {
    case "$2" in
        ''|.|..|*/*)
            echo "invalid shared-library package name '$2' from $1" >&2
            exit 1
            ;;
    esac
}

copy_payload() {
    CP_SRC=$1
    CP_REAL_SRC=$(readlink -f -- "$CP_SRC" 2>/dev/null || true)
    if [ -z "$CP_REAL_SRC" ] || [ ! -f "$CP_REAL_SRC" ]; then
        echo "shared-library package input does not resolve to a readable file: $CP_SRC" >&2
        exit 1
    fi

    CP_REAL_NAME=$(basename -- "$CP_REAL_SRC")
    validate_name "$CP_SRC" "$CP_REAL_NAME"

    CP_DEST=$DEST_DIR/$CP_REAL_NAME
    # Never follow a stale destination symlink while publishing the payload.
    rm -f -- "$CP_DEST"
    cp -f -- "$CP_REAL_SRC" "$CP_DEST"
    printf '%s\n' "$CP_REAL_NAME"
}

link_name() {
    LINK_SRC=$1
    LINK_NAME=$2
    LINK_TARGET=$3
    validate_name "$LINK_SRC" "$LINK_NAME"
    validate_name "$LINK_SRC" "$LINK_TARGET"

    [ "$LINK_NAME" = "$LINK_TARGET" ] && return 0

    LINK_DEST=$DEST_DIR/$LINK_NAME
    # Recreate aliases as package-local relative links. This preserves the
    # normal ELF linker/SONAME layout without retaining absolute producer paths
    # or duplicating the library payload for every alias.
    rm -f -- "$LINK_DEST"
    ln -s -- "$LINK_TARGET" "$LINK_DEST"
}

for SRC in "$@"; do
    if [ ! -f "$SRC" ]; then
        echo "shared-library package input is not a readable file: $SRC" >&2
        exit 1
    fi

    BASE=$(basename -- "$SRC")
    validate_name "$SRC" "$BASE"
    REAL_NAME=$(copy_payload "$SRC")

    link_name "$SRC" "$BASE" "$REAL_NAME"

    # Runtime lookup uses DT_SONAME, not necessarily the linker/source basename.
    # readelf follows source symlinks, which is exactly the artifact we package.
    SONAME=$(readelf -d "$SRC" 2>/dev/null |
        sed -n 's/.*Library soname: \[\([^]]*\)\].*/\1/p' |
        head -n 1)
    if [ -n "$SONAME" ]; then
        link_name "$SRC" "$SONAME" "$REAL_NAME"
    fi
done
