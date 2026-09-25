#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-ci}"
case "$MODE" in
    ci|host|all) ;;
    *)
        echo "usage: $0 [ci|host|all]" >&2
        exit 2
        ;;
esac

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$ROOT"

echo "Published native-base generations are immutable; repair will publish a fresh generation." >&2
exec "$ROOT/tools/publish-native-base.sh" "$MODE"
