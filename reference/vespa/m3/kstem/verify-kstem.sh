#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../../.." && pwd)
frozen="$here/frozen"
build="$here/build"

for file in corpus.txt expected.tsv PINNED_SOURCE.txt FROZEN.sha256; do
    [[ -f "$frozen/$file" ]] || {
        echo "Missing frozen/$file." >&2
        echo "A maintainer must run freeze-kstem.sh once against the pinned Vespa source and commit the results." >&2
        exit 2
    }
done

(
    cd "$frozen"
    sha256sum -c FROZEN.sha256
)

# The expected table must correspond one-for-one with the frozen corpus.
corpus_rows=$(wc -l < "$frozen/corpus.txt")
expected_rows=$(wc -l < "$frozen/expected.tsv")
[[ "$corpus_rows" -eq "$expected_rows" ]] || {
    echo "Frozen KStem row-count mismatch: corpus=$corpus_rows expected=$expected_rows" >&2
    exit 1
}

mkdir -p "$build"
rm -f "$build/kstem-rust" "$build/actual-rust.tsv"

# Compile only the actual checked-in Rust KStem implementation. This deliberately
# bypasses Cargo so KStem parity does not require the native Vespa SDK.
rustc --edition=2021 -O "$here/rust/kstem-rust.rs" -o "$build/kstem-rust"
"$build/kstem-rust" < "$frozen/corpus.txt" > "$build/actual-rust.tsv"
python3 "$here/tools/compare.py" "$frozen/expected.tsv" "$build/actual-rust.tsv"

# Optional stronger check: prove the frozen golden still equals the currently
# pinned Java source. This is intentionally opt-in so ordinary verification does
# not require a JDK or a full Vespa checkout.
if [[ "${KSTEM_VERIFY_JAVA_ORACLE:-0}" == "1" ]]; then
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

    vespa_src=$(find_vespa_src) || { echo "Set VESPA_SRC for Java oracle verification" >&2; exit 2; }
    kstemmer="$vespa_src/linguistics/src/main/java/com/yahoo/language/simple/kstem/KStemmer.java"
    recorded_hash=$(awk -F= '$1=="kstemmer_sha256" {print $2}' "$frozen/PINNED_SOURCE.txt")
    actual_hash=$(sha256sum "$kstemmer" | awk '{print $1}')
    [[ "$recorded_hash" == "$actual_hash" ]] || {
        echo "Pinned KStemmer.java changed: frozen=$recorded_hash current=$actual_hash" >&2
        exit 1
    }

    classes=$(VESPA_SRC="$vespa_src" KSTEM_JAVA_BUILD_DIR="$build/java" "$here/oracle/build-java-oracle.sh")
    java -cp "$classes" KStemOracle < "$frozen/corpus.txt" > "$build/actual-java.tsv"
    python3 "$here/tools/compare.py" "$frozen/expected.tsv" "$build/actual-java.tsv"
fi
