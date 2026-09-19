# M10 source trace

Pinned Vespa source:

```text
vespa-engine/vespa@2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

`reference/vespa/PINNED_VESPA.json` must contain the same commit when the verifier is frozen.

## Request-side ranking state

Pinned source areas:

```text
searchlib/src/vespa/searchlib/engine/request.h
searchlib/src/vespa/searchlib/engine/searchrequest.h
searchlib/src/vespa/searchlib/engine/propertiesmap.h
searchlib/src/vespa/searchlib/common/mapnames.*
searchlib/src/vespa/searchlib/fef/properties.*
```

The direct oracle must use the real request fields for:

```text
ranking profile
rank properties
feature/query inputs
feature-dump/match-feature request state where required
```

## Ranking setup and execution

Trace the pinned implementation through:

```text
searchcore/src/vespa/searchcore/proton/matching/
searchlib/src/vespa/searchlib/fef/
searchlib/src/vespa/searchlib/features/
```

Relevant behavior includes:

```text
rank profile setup
first-phase expression compilation/evaluation
bm25(field)
attribute(field)
query(name)
nativeRank/nativeFieldMatch/nativeProximity
```

## Feature results

Pinned result structures:

```text
searchlib/src/vespa/searchlib/engine/searchreply.h
vespalib/src/vespa/vespalib/util/featureset.h
```

M10 reads native hit metrics and `SearchReply::match_features`.

The feature-value stabilization scenario additionally traces the exact value
representation through:

```text
vespalib/src/vespa/vespalib/util/featureset.h
searchlib/src/vespa/searchlib/engine/searchreply.h
searchlib/src/vespa/searchlib/features/attributefeature.cpp
searchlib/src/vespa/searchlib/features/queryfeature.cpp
searchlib/src/vespa/searchlib/features/queryfeature.h
searchcore/src/vespa/searchcore/proton/matching/extract_features.cpp
searchcore/src/tests/proton/matching/matching_test.cpp
```

The candidate `attribute(embedding)` was tested against the pinned live
oracle, but this revision rejects extracting a tensor attribute as a match
feature (`DenseTensorAttribute` reports that the operation is not
implemented). The frozen scenario therefore uses the deterministic
`query(embedding)` feature instead. `QueryBlueprint` accepts a tensor query
value, creates an object-valued feature executor, and the matcher stores that
object in `FeatureSet::Value` with `set_data`. The oracle records `as_data()`
byte-for-byte and intentionally performs no tensor decoding. The pinned
revision is
`2300ef0412bdb76f227ae3b497c8ca5e73c5da4e`.

## Rank-profile config oracle

For the frozen config syntax, resolve the pinned config-model derivation around raw rank profiles and FEF properties. Important generated property names include:

```text
vespa.rank.firstphase
vespa.match.feature
```

The verifier-owned fixture is authoritative after it has been generated/reviewed against the pinned revision.
