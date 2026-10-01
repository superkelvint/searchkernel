#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 || -z "$1" || "$1" == / ]]; then
    echo "usage: $0 <temporary-tree>" >&2
    exit 2
fi

tree=$1
[[ -e "$tree" || -L "$tree" ]] || exit 0

# Native-base cache trees are sealed read-only after installation. Packaging
# uses archive-preserving copies, so temporary staging trees can inherit those
# modes and ordinary rm -rf cannot unlink their contents.
chmod -R u+rwX -- "$tree"
rm -rf -- "$tree"
