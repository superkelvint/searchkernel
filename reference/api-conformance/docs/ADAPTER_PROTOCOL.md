# Adapter protocol — `sk-ir-conformance/v1`

The verifier starts the adapter once per case. The adapter reads one JSON object from stdin and writes exactly one JSON object to stdout.

Input:

```json
{
  "protocol": "sk-ir-conformance/v1",
  "case_id": "search.defaults",
  "operation": "search_to_ir",
  "index": {},
  "input": {}
}
```

The adapter must construct the corresponding FlatBuffers wire representation **without pre-normalizing away the case's omission/null/empty distinctions**, then invoke the named `*_to_ir` pass. Harness-only composition operations such as `write_to_ir` dispatch to one of the existing concrete FlatBuffers write requests; they do not imply a new wire envelope or backend method.

Successful output:

```json
{"ok": {}}
```

Error output is the stable comparison projection only:

```json
{
  "error": {
    "code": "invalid_argument",
    "details": [{"code": "unknown_field", "path": "filter.comparison.field"}],
    "retryable": false
  }
}
```

Human-readable error messages are intentionally excluded from golden equality. Object key order is ignored. Array order is semantic.
