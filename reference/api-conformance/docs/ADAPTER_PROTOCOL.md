# Adapter protocol — `sk-conformance/v2`

The verifier starts the adapter once per case. The adapter reads one JSON object from stdin and writes exactly one JSON object to stdout.

```json
{
  "protocol":"sk-conformance/v2",
  "case_id":"search.defaults",
  "operation":"resolve_search",
  "index":{},
  "input":{},
  "fixtures":{}
}
```

Output is either:

```json
{"ok":{}}
```

or:

```json
{"error":{"code":"unknown_field","details":{"field":"x"}}}
```

`run_stateful_scenario` is still one adapter invocation: its input contains explicit initial state plus an ordered list of steps. No external persistence or backend is required.

Object key order is ignored. Array order is semantic unless the relevant canonical contract explicitly defines a set and normalization sorts it.
