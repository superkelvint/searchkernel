from __future__ import annotations

import os
from pathlib import Path

from searchkernel import Index, Integer, Keyword, Text

path = Path(os.environ.get("SEARCHKERNEL_QUICKSTART_PATH", "./search-python"))
documents = [
    {
        "_id": "quickstart-a",
        "title": "quickstart alpha",
        "kind": "guide",
        "score": 2,
    },
    {
        "_id": "quickstart-b",
        "title": "quickstart beta",
        "kind": "guide",
        "score": 1,
    },
    {
        "_id": "quickstart-c",
        "title": "quickstart other",
        "kind": "other",
        "score": 0,
    },
]


def matching_ids(index: Index) -> list[str]:
    return [
        hit.id
        for hit in (
            index.search("quickstart")
            .filter("kind = 'guide'")
            .sort("score", "asc")
            .execute()
            .hits
        )
    ]


with Index.create(
    path,
    fields={
        "title": Text(stored=True, searchable=True),
        "kind": Keyword(stored=True, filterable=True),
        "score": Integer(stored=True, sortable=True),
    },
) as index:
    for document in documents:
        index.insert(document)
    assert matching_ids(index) == ["quickstart-b", "quickstart-a"]

with Index.open(path) as index:
    assert index.get("quickstart-b")["score"] == 1
    assert matching_ids(index) == ["quickstart-b", "quickstart-a"]

print("Python release quickstart PASS")
