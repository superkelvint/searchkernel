#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../../../.." && pwd)

find_vespa_src() {
    if [[ -n "${VESPA_SRC:-}" && -f "$VESPA_SRC/linguistics/src/main/java/com/yahoo/language/simple/kstem/KStemmer.java" ]]; then
        printf '%s\n' "$VESPA_SRC"
        return 0
    fi

    local candidate
    for candidate in \
        "$repo/upstream/vespa" \
        "$repo/reference/vespa" \
        /work/upstream/vespa; do
        if [[ -f "$candidate/linguistics/src/main/java/com/yahoo/language/simple/kstem/KStemmer.java" ]]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done

    while IFS= read -r candidate; do
        if [[ -f "$candidate/linguistics/src/main/java/com/yahoo/language/simple/kstem/KStemmer.java" ]]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done < <(find "$repo/reference" -maxdepth 2 -type d -name 'vespa*' 2>/dev/null | sort)

    return 1
}

vespa_src=$(find_vespa_src) || {
    echo "Unable to locate pinned Vespa source." >&2
    echo "Set VESPA_SRC=/path/to/pinned/vespa." >&2
    exit 2
}

src="$vespa_src/linguistics/src/main/java/com/yahoo/language/simple/kstem"
build="${KSTEM_JAVA_BUILD_DIR:-$here/build}"
classes="$build/classes"
rm -rf "$classes"
mkdir -p "$classes"

mapfile -t java_sources < <(find "$src" -maxdepth 1 -type f -name '*.java' | sort)
if [[ ${#java_sources[@]} -eq 0 ]]; then
    echo "No KStem Java sources found under $src" >&2
    exit 2
fi

javac -encoding UTF-8 -d "$classes" "${java_sources[@]}" "$here/KStemOracle.java"
printf '%s\n' "$classes"
