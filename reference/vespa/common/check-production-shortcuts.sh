#!/bin/sh
set -eu

REPO_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)

# Known golden IDs must not appear in production Rust implementation. They may
# appear only in reference/vespa test drivers and fixtures.
if [ -d "$REPO_ROOT/crates" ]; then
    if grep -R -n -E 'id:test:typea::1|id:test:typea::2' "$REPO_ROOT/crates" \
        --include='*.rs' >/tmp/searchkernel-shortcut-scan.txt 2>/dev/null; then
        echo "golden fixture IDs found in production Rust source:" >&2
        cat /tmp/searchkernel-shortcut-scan.txt >&2
        exit 1
    fi
fi
