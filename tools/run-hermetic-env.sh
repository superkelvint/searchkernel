#!/usr/bin/env bash
set -euo pipefail

die() {
    echo "error: $*" >&2
    exit 2
}

declare -a assignments=()
while (($#)); do
    case "$1" in
        --)
            shift
            break
            ;;
        *=*)
            name=${1%%=*}
            [[ "$name" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] ||
                die "invalid environment assignment name: $name"
            assignments+=("$1")
            shift
            ;;
        *)
            die "expected NAME=VALUE or -- before command, found: $1"
            ;;
    esac
done

(($#)) || die "missing command after --"

# Synthetic tests must declare every environment input they consume. Starting
# from an empty environment prevents CI, developer shells, and nested tooling
# from silently changing fixture semantics through ambient overrides.
exec /usr/bin/env -i "${assignments[@]}" "$@"
