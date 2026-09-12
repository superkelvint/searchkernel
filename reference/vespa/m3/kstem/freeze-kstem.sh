#!/usr/bin/env bash
# Maintainer command: creates the immutable KStem corpus + Vespa golden output.
# Do NOT call this from verify-kstem.sh.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../../.." && pwd)

find_vespa_src() {
    if [[ -n "${VESPA_SRC:-}" && -f "$VESPA_SRC/linguistics/src/main/java/com/yahoo/language/simple/kstem/KStemmer.java" ]]; then
        printf '%s\n' "$VESPA_SRC"; return 0
    fi
    local c
    for c in "$repo/upstream/vespa" "$repo/reference/vespa" /work/upstream/vespa; do
        [[ -f "$c/linguistics/src/main/java/com/yahoo/language/simple/kstem/KStemmer.java" ]] && { printf '%s\n' "$c"; return 0; }
    done
    while IFS= read -r c; do
        [[ -f "$c/linguistics/src/main/java/com/yahoo/language/simple/kstem/KStemmer.java" ]] && { printf '%s\n' "$c"; return 0; }
    done < <(find "$repo/reference" -maxdepth 2 -type d -name 'vespa*' 2>/dev/null | sort)
    return 1
}

vespa_src=$(find_vespa_src) || { echo "Set VESPA_SRC to the pinned Vespa checkout" >&2; exit 2; }
kstemmer="$vespa_src/linguistics/src/main/java/com/yahoo/language/simple/kstem/KStemmer.java"
mkdir -p "$here/frozen"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

python3 "$here/tools/build-corpus.py" --vespa-src "$vespa_src" --out "$tmp/corpus.txt"
classes=$(VESPA_SRC="$vespa_src" KSTEM_JAVA_BUILD_DIR="$tmp/java" "$here/oracle/build-java-oracle.sh")
java -cp "$classes" KStemOracle < "$tmp/corpus.txt" > "$tmp/expected.tsv"

commit=unknown
if git -C "$vespa_src" rev-parse HEAD >/dev/null 2>&1; then
    commit=$(git -C "$vespa_src" rev-parse HEAD)
fi
{
    printf 'vespa_commit=%s\n' "$commit"
    printf 'kstemmer_sha256=%s\n' "$(sha256sum "$kstemmer" | awk '{print $1}')"
} > "$tmp/PINNED_SOURCE.txt"

mv "$tmp/corpus.txt" "$here/frozen/corpus.txt"
mv "$tmp/expected.tsv" "$here/frozen/expected.tsv"
mv "$tmp/PINNED_SOURCE.txt" "$here/frozen/PINNED_SOURCE.txt"
(
    cd "$here/frozen"
    sha256sum corpus.txt expected.tsv PINNED_SOURCE.txt > FROZEN.sha256
)

printf 'Frozen KStem corpus and Vespa golden under %s/frozen\n' "$here"
wc -l "$here/frozen/corpus.txt" "$here/frozen/expected.tsv"
