# M13 source trace

Pinned Vespa revision:

```text
2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

The source-level boundary is defined in:

```text
upstream/vespa/vespalib/src/vespa/vespalib/util/featureset.h
```

`FeatureSet::Value` stores either a `double` or a byte vector. Its
`is_double()`, `is_data()`, `as_double()`, and `as_data()` operations establish
the DOUBLE/DATA distinction used by this milestone.

The reply carries those values through `FeatureValues` in:

```text
upstream/vespa/searchlib/src/vespa/searchlib/engine/searchreply.h
```

M13 therefore freezes opaque DATA transport at the Search Kernel boundary. It
does not treat Vespa's feature value as a tensor and does not require tensor
decoding to establish the ABI contract. The oracle-backed normative verifier
for this boundary is the M10 feature-values verifier; M13 is supplemental
smoke/regression coverage only.
