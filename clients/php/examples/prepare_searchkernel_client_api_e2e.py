from __future__ import annotations

from pathlib import Path
import sys

from searchkernel import Boolean, Float, Index, Integer, Keyword, RankProfile, TensorField, Text

VECTOR_TYPE = "tensor<float>(x[8])"

if len(sys.argv) != 2:
    raise SystemExit("usage: prepare_searchkernel_client_api_e2e.py INDEX_PATH")

path = Path(sys.argv[1])
with Index.create(
    path,
    fields={
        "title": Text(stored=True, searchable=True),
        "brand": Keyword(stored=True, filterable=True, sortable=True, facetable=True),
        "category": Keyword(stored=True, filterable=True, facetable=True),
        "price": Float(stored=True, filterable=True, sortable=True),
        "sale_price": Float(stored=True, filterable=True, sortable=True),
        "rating": Integer(stored=True, filterable=True, sortable=True),
        "inventory": Integer(stored=True, filterable=True, sortable=True),
        "in_stock": Boolean(stored=True, filterable=True),
        "embedding": TensorField(VECTOR_TYPE, stored=True),
    },
    rank_profiles=[
        RankProfile("boosted")
        .first_phase("query(boost)")
        .match_feature("query(boost)"),
        RankProfile("semantic")
        .first_phase("closeness(embedding)")
        .query_input("query_embedding", VECTOR_TYPE)
        .match_feature("query(query_embedding)"),
        RankProfile("hybrid")
        .first_phase("bm25(title) + 10 * closeness(embedding)")
        .query_input("query_embedding", VECTOR_TYPE)
        .match_feature("bm25(title)")
        .match_feature("closeness(embedding)")
        .match_feature("query(query_embedding)"),
        RankProfile("two_phase")
        .first_phase("attribute(rating)")
        .second_phase("attribute(inventory)")
        .rerank_count(3),
        RankProfile("first_phase_only").first_phase("attribute(rating)"),
    ],
):
    pass
