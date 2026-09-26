#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
TMP=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-shell-syntax.XXXXXX")
trap 'rm -rf "$TMP"' EXIT

git -C "$TMP" init -q
git -C "$TMP" config user.email test@example.invalid
git -C "$TMP" config user.name test

mkdir -p "$TMP/tools"
cp "$ROOT/tools/check-shell-syntax.sh" "$TMP/tools/check-shell-syntax.sh"

cat > "$TMP/good.sh" <<'EOF'
#!/usr/bin/env bash
if [[ -n "ok" ]]; then
    printf '%s\n' ok
fi
EOF

cat > "$TMP/extensionless" <<'EOF'
#!/bin/sh
case "x" in
    x) printf '%s\n' ok ;;
esac
EOF

cat > "$TMP/env-sh" <<'EOF'
#!/usr/bin/env sh
case "x" in
    x) printf '%s\n' ok ;;
esac
EOF

cat > "$TMP/bad.sh" <<'EOF'
#!/usr/bin/env bash
if [[ -x /tmp/example --not-part-of-the-test ]]; then
    :
fi
EOF

git -C "$TMP" add tools/check-shell-syntax.sh good.sh extensionless env-sh bad.sh

if bash "$TMP/tools/check-shell-syntax.sh" "$TMP" >/dev/null 2>&1; then
    echo "shell syntax regression failed: malformed Bash was accepted" >&2
    exit 1
fi

git -C "$TMP" rm -q --cached bad.sh
bash "$TMP/tools/check-shell-syntax.sh" "$TMP" >/dev/null

echo "shell syntax regression PASS"
