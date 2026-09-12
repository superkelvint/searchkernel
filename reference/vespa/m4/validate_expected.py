#!/usr/bin/env python3
import json, sys
from pathlib import Path

rows = {o["id"]: o for o in (json.loads(x) for x in Path(sys.argv[1]).read_text().splitlines() if x.strip())}

def ids(k): return [h["id"] for h in rows[k]["hits"]]

def require(cond, msg):
    if not cond: raise SystemExit(msg)

# Semantic invariants we can assert without computing Vespa behavior.
require("doc1" in ids("term-brown-initial"), "brown must match doc1")
require("doc3" in ids("term-brown-initial"), "brown must match doc3")
require("doc1" in ids("phrase-brown-fox-initial"), "brown fox phrase must match doc1")
require("doc3" not in ids("phrase-brown-fox-initial"), "phrase test failed: doc3 is non-adjacent")
require("doc5" in ids("m3-running-uppercase"), "M3 integration probe did not match doc5")
require(ids("term-red-after-update") == [], "old term red remains after update")
require("doc2" in ids("phrase-brown-fox-after-update"), "updated doc2 must match phrase")
require("doc1" not in ids("phrase-brown-fox-after-delete"), "deleted doc1 remains in phrase results")
require(ids("term-quick-after-delete") == [], "deleted doc1 quick posting remains")
bm25_hits = rows["bm25-fox-initial"]["hits"]
require(bm25_hits, "BM25 checkpoint has no hits")
require(all("score" in h for h in bm25_hits), "BM25 checkpoint lacks scores")
require(bm25_hits[0]["id"] == "doc4", "BM25 term-frequency probe: repeated-fox doc4 must rank first")
require(len({round(float(h["score"]), 12) for h in bm25_hits}) > 1,
        "BM25 checkpoint scores are all identical; bm25 profile may not be active")
print("M4 oracle semantic sanity checks passed")
