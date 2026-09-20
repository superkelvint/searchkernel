# M12 oracle output protocol

For a positive scenario the oracle writes exactly one JSON object to stdout:

```json
{
  "id": "scenario-id",
  "steps": [
    {
      "id": "step-id",
      "hits": ["id:ns:typea::x", "id:ns:typea::mid", "id:ns:typea::y"]
    }
  ]
}
```

No logging belongs on stdout. Diagnostics go to stderr.

The output intentionally excludes:

- score bits;
- total hit count;
- coverage;
- latency;
- HNSW state.

M12 v1 freezes only exact ordered retrieval semantics.
