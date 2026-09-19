#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
here=$(c6_here); root=$(c6_root); binary=$(bindgen_binary)
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
out1="$tmp/out1"; out2="$tmp/out2"; mkdir -p "$out1" "$out2"
bfbs_root="$root/core/crates/searchkernel-json/bfbs"
run_generate() { "$binary" generate --bfbs-root "$bfbs_root" --output-root "$1"; }
run_generate "$out1"; run_generate "$out2"
diff -ru --strip-trailing-cr "$out1" "$out2"
sed '/^[[:space:]]*#/d;/^[[:space:]]*$/d' "$here/GENERATED_PATHS.txt" | sort > "$tmp/expected-paths"
find "$out1" -type f -printf '%P\n' | sort > "$tmp/actual-paths"
diff -u "$tmp/expected-paths" "$tmp/actual-paths"
while IFS= read -r relative; do
  [[ -z "$relative" || "$relative" == \#* ]] && continue
  committed="$root/$relative"; first="$out1/$relative"
  [[ -e "$committed" ]] || { echo "committed generated path missing: $relative" >&2; exit 1; }
  [[ -e "$first" ]] || { echo "generator did not emit: $relative" >&2; exit 1; }
  diff -ru --strip-trailing-cr "$committed" "$first"
done < "$here/GENERATED_PATHS.txt"
echo "C6 deterministic generation PASS"
