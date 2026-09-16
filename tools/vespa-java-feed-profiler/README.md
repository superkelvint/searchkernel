# Java Vespa feed profiler

This is a small Java 11+ benchmark for the Vespa `/document/v1` API. It uses
only the JDK HTTP client, so it does not require Maven or a Vespa SDK build.

Each `PUT` is timed from immediately before the HTTP request until the Vespa
response is received. A successful response is treated as the operation's
acknowledgment boundary. The report also includes the wall-clock interval from
the synchronized worker start until every requested operation has received a
response.

This measures Java client-to-Vespa feeding, not embedded `DocumentDB::open()`.
The Vespa application and document schema must already be deployed and online.

## Example

```bash
tools/vespa-java-feed-profiler/run.sh \
  --endpoint http://localhost:8080/document/v1 \
  --namespace profile \
  --document-type typea \
  --count 20 \
  --concurrency 1 \
  --text-field title \
  --integer-field qty
```

The tool writes one JSON report to stdout. Document IDs include a timestamped
prefix by default, so repeated runs do not overwrite earlier documents. Use
`--id-prefix` when a fixed prefix is desired.

Use `--concurrency` to compare serialized acknowledgments with a pipelined
feed. The `ack_latency_ms` values are per-document response latencies; the
`all_acknowledgments_ms` value is the end-to-end feed interval.
