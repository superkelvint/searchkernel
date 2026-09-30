from __future__ import annotations

import os
from pathlib import Path

from searchkernel import Index, Integer, Keyword, Text

path = Path(os.environ.get("SEARCHKERNEL_QUICKSTART_PATH", "./search-python"))

with Index.create(
    path,
    fields={
        "title": Text(stored=True, searchable=True),
        "kind": Keyword(stored=True, filterable=True),
        "score": Integer(stored=True, sortable=True),
    },
) as index:
    index.insert(
        {
            "_id": "quickstart-python",
            "title": "quickstart example",
            "kind": "quickstart",
            "score": 1,
        }
    )
    hits = index.search("quickstart").execute().hits
    assert [hit.id for hit in hits] == ["quickstart-python"]

with Index.open(path) as index:
    assert index.get("quickstart-python")["title"] == "quickstart example"

print("Python release quickstart PASS")
