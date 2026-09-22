#!/bin/sh
set -eu

if [ "$#" -lt 2 ]; then
    echo "usage: $0 DEST_LIB_DIR SOURCE..." >&2
    exit 2
fi

DEST_DIR=$1
shift
mkdir -p "$DEST_DIR"

copy_name() {
    SRC=$1
    NAME=$2
    case "$NAME" in
        ''|.|..|*/*)
            echo "invalid shared-library package name '$NAME' from $SRC" >&2
            exit 1
            ;;
    esac

    DEST=$DEST_DIR/$NAME
    # cp -f may follow an existing destination symlink. Remove the name first
    # so packaging can never overwrite a host file through a stale/hostile link.
    rm -f -- "$DEST"
    cp -L -f -- "$SRC" "$DEST"
}

for SRC in "$@"; do
    if [ ! -f "$SRC" ]; then
        echo "shared-library package input is not a readable file: $SRC" >&2
        exit 1
    fi

    BASE=$(basename -- "$SRC")
    copy_name "$SRC" "$BASE"

    # Runtime lookup uses DT_SONAME, not necessarily the linker/source basename.
    # readelf follows source symlinks, which is exactly the artifact we package.
    SONAME=$(readelf -d "$SRC" 2>/dev/null |
        sed -n 's/.*Library soname: \[\([^]]*\)\].*/\1/p' |
        head -n 1)
    if [ -n "$SONAME" ] && [ "$SONAME" != "$BASE" ]; then
        copy_name "$SRC" "$SONAME"
    fi
done
