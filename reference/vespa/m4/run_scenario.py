#!/usr/bin/env python3
import argparse
import json
import math
import os
import secrets
import subprocess
import sys
from pathlib import Path

from prepare_m3 import analyze


def hx(s: str) -> str:
    return s.encode("utf-8").hex()


def compact_document_analysis(obj):
    """Encode M3 document analysis for the C++ oracle.

    Each record is position,byte_start,byte_len,term_hex. Document spans are
    required because M4 must construct Vespa linguistic annotations over the
    original StringFieldValue.
    """
    records = []
    for term in obj.get("terms", []):
        records.append(
            f"{int(term['position'])},{int(term['byte_start'])},{int(term['byte_len'])},"
            f"{term['term'].encode('utf-8').hex()}"
        )
    return ";".join(records)


def compact_query_analysis(obj):
    """Encode M3 query analysis for the C++ oracle.

    M3 query terms intentionally contain only {stems[], position}; they do not
    have document source spans. M4's initial term/phrase corpus requires one
    effective Vespa stem per logical query position. Refuse multi-stem output
    rather than silently discarding alternatives.
    """
    records = []
    for term in obj.get("terms", []):
        stems = term.get("stems", [])
        if len(stems) != 1:
            raise RuntimeError(
                f"M4 query corpus requires exactly one stem per position; "
                f"position={term.get('position')!r} stems={stems!r}"
            )
        records.append(
            f"{int(term['position'])},{stems[0].encode('utf-8').hex()}"
        )
    return ";".join(records)


def load_scenario(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def prepare_analysis(scenario):
    docs = []
    queries = []
    for doc_id, text in scenario["documents"].items():
        docs.append((f"doc-{doc_id}", text))
    for step in scenario["steps"]:
        if step["op"] == "update":
            docs.append((f"update-{step['doc']}", step["text"]))
        elif step["op"] == "search":
            queries.append((f"query-{step['id']}", step["query"]["text"]))
    opts = scenario["linguistics"]
    doc_a = analyze("document", docs, opts["stemming"], bool(opts["remove_accents"]))
    query_a = analyze("query", queries, opts["stemming"], bool(opts["remove_accents"]))
    return doc_a, query_a


def parse_result(line: str, expected_id: str, aliases):
    parts = line.rstrip("\n").split("\t")
    if len(parts) != 3 or parts[0] != "RESULT" or parts[1] != expected_id:
        raise RuntimeError(f"invalid RESULT for {expected_id}: {line!r}")
    hits = []
    payload = parts[2]
    if payload:
        for pair in payload.split(";"):
            native_id, score_s = pair.rsplit(",", 1)
            alias = aliases.get(native_id)
            if alias is None:
                raise RuntimeError(f"unknown native document id {native_id!r}")
            score = float(score_s)
            if not math.isfinite(score):
                raise RuntimeError(f"non-finite score for {native_id}: {score_s}")
            hits.append((alias, score))
    return hits


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mode", choices=("oracle", "rust"), required=True)
    ap.add_argument("--scenario", type=Path, required=True)
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("command", nargs=argparse.REMAINDER)
    args = ap.parse_args()
    if not args.command:
        ap.error("missing driver command")

    scenario = load_scenario(args.scenario)
    doc_analysis = query_analysis = None
    if args.mode == "oracle":
        doc_analysis, query_analysis = prepare_analysis(scenario)

    # Salt physical ids so production code never receives the frozen aliases.
    salt = os.environ.get("M4_ID_SALT") or secrets.token_hex(8)
    physical = {alias: f"id:m4:typea::{salt}-{alias}" for alias in scenario["documents"]}
    aliases = {v: k for k, v in physical.items()}

    p = subprocess.Popen(args.command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=sys.stderr, text=True, bufsize=1)
    assert p.stdin and p.stdout

    out = []
    documents = dict(scenario["documents"])
    for step in scenario["steps"]:
        op = step["op"]
        if op in ("put", "update"):
            alias = step["doc"]
            text = step.get("text", documents[alias])
            documents[alias] = text
            if args.mode == "oracle":
                key = f"doc-{alias}" if op == "put" else f"update-{alias}"
                analysis_spec = compact_document_analysis(doc_analysis[key])
                line = f"{op.upper()}\t{physical[alias]}\t{hx(text)}\t{hx(analysis_spec)}\n"
            else:
                line = f"{op.upper()}\t{physical[alias]}\t{hx(text)}\n"
            p.stdin.write(line); p.stdin.flush()
            reply = p.stdout.readline().rstrip("\n")
            if reply != "OK":
                raise RuntimeError(f"{op} failed: {reply!r}")
        elif op == "delete":
            alias = step["doc"]
            p.stdin.write(f"DELETE\t{physical[alias]}\n"); p.stdin.flush()
            reply = p.stdout.readline().rstrip("\n")
            if reply != "OK":
                raise RuntimeError(f"delete failed: {reply!r}")
        elif op == "search":
            sid = step["id"]
            kind = step["query"]["kind"]
            rank = step.get("rank", "default")
            raw = step["query"]["text"]
            if args.mode == "oracle":
                analysis_spec = compact_query_analysis(query_analysis[f"query-{sid}"])
                line = f"{kind.upper()}\t{sid}\t{rank}\t{hx(analysis_spec)}\n"
            else:
                line = f"{kind.upper()}\t{sid}\t{rank}\t{hx(raw)}\n"
            p.stdin.write(line); p.stdin.flush()
            hits = parse_result(p.stdout.readline(), sid, aliases)
            obj = {"id": sid, "hits": []}
            for alias, score in hits:
                hit = {"id": alias}
                if step.get("include_scores", False):
                    hit["score"] = score
                obj["hits"].append(hit)
            out.append(json.dumps(obj, ensure_ascii=False, separators=(",", ":")))
        else:
            raise RuntimeError(f"unknown op {op!r}")

    p.stdin.write("QUIT\n"); p.stdin.flush()
    reply = p.stdout.readline().rstrip("\n")
    if reply != "OK":
        raise RuntimeError(f"quit failed: {reply!r}")
    p.stdin.close()
    rc = p.wait()
    if rc != 0:
        raise SystemExit(rc)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("\n".join(out) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()

