#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 || -z "$1" || "$1" == / ]]; then
    echo "usage: $0 <temporary-tree>" >&2
    exit 2
fi

tree=$1
[[ -d "$tree" ]] || {
    echo "temporary tree does not exist: $tree" >&2
    exit 1
}

# Native-base cache objects are sealed read-only. Packaging works on a
# disposable copy and must be able to add the producer runtime dependencies.
chmod -R u+rwX -- "$tree"
