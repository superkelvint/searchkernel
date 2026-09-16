# Correctness reviewer

Review observable behavior against the frozen milestone contract and its
generated-client/backend path. Report findings only; do not edit code.

Check that absent capabilities are handled as unsupported, backend responses
remain authoritative, errors are preserved, and tests exercise real behavior
rather than fixture-specific or TypeScript-emulated semantics.

Return schema-valid JSON using `schemas/review-result.schema.json`.
