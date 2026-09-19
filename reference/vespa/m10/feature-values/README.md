# M10 feature-value verification

This additive scenario freezes the pinned Vespa `FeatureSet::Value` boundary:
`attribute(ordinal)` is emitted as a DOUBLE and tensor-valued `query(embedding)`
is emitted as opaque DATA. The exact DATA bytes are compared across the direct
Vespa oracle, the native C ABI, and Rust `MatchFeatureValue::Data`.

The pinned revision does not support extracting a tensor attribute directly as
a match feature, so the scenario uses the equivalent deterministic query tensor
feature. No verifier component decodes the payload.

Run it with:

```text
reference/vespa/m10/feature-values/verify-feature-values.sh
```
