from __future__ import annotations

import json
from pathlib import Path
import sys

from searchkernel import Boolean, Float, Index, Integer, Keyword, RankProfile, TensorField, Text

ROOT = Path(__file__).resolve().parents[3]
CORPUS_PATH = ROOT / "reference/client-real-e2e-parity/corpus.json"
CORPUS = json.loads(CORPUS_PATH.read_text())
FIELD_TYPES = {
    "text": Text,
    "keyword": Keyword,
    "int64": Integer,
    "float64": Float,
    "bool": Boolean,
}


def field_from_spec(spec: dict[str, object]):
    field_type = str(spec["type"])
    options = {key: value for key, value in spec.items() if key != "type"}
    if field_type.startswith("tensor<"):
        return TensorField(field_type, **options)
    return FIELD_TYPES[field_type](**options)


def rank_profile_from_spec(spec: dict[str, object]) -> RankProfile:
    profile = RankProfile(str(spec["name"])).first_phase(str(spec["first_phase"]))
    if "second_phase" in spec:
        profile = profile.second_phase(str(spec["second_phase"]))
    if "rerank_count" in spec:
        profile = profile.rerank_count(int(spec["rerank_count"]))
    for name, type_spec in dict(spec.get("query_inputs", {})).items():
        profile = profile.query_input(str(name), str(type_spec))
    for feature in spec.get("match_features", []):
        profile = profile.match_feature(str(feature))
    return profile


if len(sys.argv) != 2:
    raise SystemExit("usage: prepare_searchkernel_client_api_e2e.py INDEX_PATH")

path = Path(sys.argv[1])
schema = CORPUS["schema"]
with Index.create(
    path,
    fields={name: field_from_spec(spec) for name, spec in schema["fields"].items()},
    rank_profiles=[rank_profile_from_spec(spec) for spec in schema["rank_profiles"]],
):
    pass
