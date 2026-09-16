# I-Series — Full product-path conformance

This corpus is the release gate for the integrated Rust product path. Every
case runs through the public `searchkernel` API and the real native engine.

The driver must not import the IR adapter, low-level Vespa engine, native
bindings, scripted backends, or mocks. The checked-in expected output is the
semantic contract. Errors compare their public code, retryability, and
structured detail code/path/metadata; volatile human-readable native messages
are diagnostic only.

The positive trace covers lifecycle, CRUD, patch/upsert replacement, text and
phrase search, boolean filters, comparisons, ranges, existence, projection,
pagination, all product sort and facet modes, totals, scores, analysis,
describe, inspect, timeout propagation, close, and reopen.

Negative cases cover every currently reachable unsupported product option and
schema capability, plus representative invalid field-use errors. Adding a
public option requires adding a positive or negative case here before the
release gate can pass.
