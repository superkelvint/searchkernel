#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M4="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="$(cd "$M4/../../.." && pwd)"
VESPA="${VESPA_SOURCE_DIR:-$ROOT/upstream/vespa}"
BASE="$VESPA/searchcore/src/tests/proton/documentdb/cfg"
OUT="$M4/fixture/embedded_text_cfg"
PINNED="2300ef0412bdb76f227ae3b497c8ca5e73c5da4e"

[[ -d "$BASE" ]] || { echo "missing pinned Vespa documentdb fixture: $BASE" >&2; exit 1; }
actual="$(git -C "$VESPA" rev-parse HEAD)"
[[ "$actual" == "$PINNED" ]] || {
  echo "M4 oracle expects Vespa $PINNED, found $actual" >&2
  exit 1
}

# Once created, this directory becomes part of M4-IMMUTABLE.sha256. Do not
# regenerate it during implementation/acceptance.
if [[ -f "$OUT/.m4-text-fixture" ]]; then
  exit 0
fi
if [[ -e "$OUT" && -n "$(find "$OUT" -mindepth 1 -maxdepth 1 -print -quit 2>/dev/null)" ]]; then
  echo "refusing to overwrite non-empty M4 fixture: $OUT" >&2
  exit 1
fi

rm -rf "$OUT"
mkdir -p "$OUT"
cp -a "$BASE/." "$OUT/"

# M4 has no attributes. The source cfg's attr1 belongs to the M0 integer spike.
: > "$OUT/attributes.cfg"

# One indexed string field. Positions are required for phrase queries.
# Interleaved features are enabled because Vespa's BM25 feature consumes field
# length/occurrence information from the text index.
cat > "$OUT/indexschema.cfg" <<'CFG'
indexfield[1]
indexfield[0].name "title"
indexfield[0].datatype STRING
indexfield[0].collectiontype SINGLE
indexfield[0].prefix false
indexfield[0].phrases false
indexfield[0].positions true
indexfield[0].averageelementlen 512
indexfield[0].interleavedfeatures true
CFG

# Keep a plain default profile for term/phrase membership checks and a dedicated
# profile whose first-phase rank feature is Vespa's real bm25(title).
cat > "$OUT/rank-profiles.cfg" <<'CFG'
rankprofile[2]
rankprofile[0].name "default"
rankprofile[1].name "bm25"
rankprofile[1].fef.property[1]
rankprofile[1].fef.property[0].name "vespa.rank.firstphase"
rankprofile[1].fef.property[0].value "bm25(title)"
CFG

cat > "$OUT/.m4-text-fixture" <<EOF2
source_revision=$PINNED
source_fixture=searchcore/src/tests/proton/documentdb/cfg
field=title
index=STRING,SINGLE,positions=true,interleavedfeatures=true
rank_profile=bm25 -> bm25(title)
EOF2

echo "Prepared M4 text fixture: $OUT" >&2
