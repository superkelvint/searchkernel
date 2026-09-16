# M10 — Ranking integration

## Goal

Expose Vespa's existing local ranking system through the low-level `searchkernel-vespa-engine` API without introducing a Search Kernel scoring engine.

M10 freezes a deliberately small scalar ranking surface:

```text
Schema
  -> RankProfile
      -> first-phase Vespa ranking expression
      -> match features

SearchRequest
  -> selected rank profile
  -> scalar query features
  -> request rank properties

Vespa Matcher
  -> score
  -> native match-feature values
```

Ranking execution remains entirely in Vespa C++.

## Frozen Rust API

The compile-only contract is:

```text
reference/vespa/api-contract/src/bin/m10.rs
```

M10 adds the following low-level concepts to the generic M-series API:

```text
RankProfile
Schema::rank_profile
SearchRequest::query_feature
SearchRequest::rank_property
Hit::match_features
```

The milestone must extend the generic `Document` / `SearchRequest` / `SearchResult` API. It must not add another milestone-specific search request/result family.

## Rank profiles

A rank profile has:

```text
name
first-phase ranking expression
zero or more match features
```

The expression and feature names use Vespa ranking syntax. Search Kernel validates safe structural constraints but does not parse or evaluate Vespa ranking expressions itself.

M10 v1 requires scalar ranking only. The verifier covers:

```text
bm25(field)
attribute(field)
query(name)
nativeRank(field)
nativeFieldMatch(field)
nativeProximity(field)
arithmetic combinations of scalar features
```

### Profile validation

Reject before engine execution:

```text
empty profile name
reserved/default profile redefinition
duplicate profile names
empty first-phase expression
empty match-feature expression
duplicate match-feature expressions within one profile
```

Names and expressions must also respect the wrapper's existing bounded-string/resource policy.

## BM25 and field-specific ranking

M10 must allow a profile to select BM25 for a specific indexed text field. The verifier freezes separate `bm25(title)` and `bm25(body)` profiles.

BM25 must be evaluated by Vespa. Rust must not compute term statistics, document length, inverse document frequency, or a BM25 score.

## Attribute features

Ranking expressions may read numeric attributes through Vespa features such as:

```text
attribute(popularity)
```

The current value must come from Vespa's attribute vector at match time.

M9 already proves mutable attribute visibility. M10 proves that the configured ranking expression consumes that native attribute feature.

## Query features

M10 v1 supports finite scalar `f64` query features.

A call such as:

```rust
request.query_feature("boost", 2.0)
```

must make the value available to Vespa as the input consumed by:

```text
query(boost)
```

Search Kernel does not substitute the value into an expression and does not evaluate the expression in Rust.

Reject:

```text
empty query-feature name
non-finite value
duplicate query-feature name
```

Tensor query features are outside M10 v1.

## Request rank properties

M10 exposes low-level request rank properties as string key/value pairs.

The direct request path must populate Vespa's native rank-property collection. The verifier uses a raw rank property named `boost` and proves that the same `query(boost)` feature receives the same value as the typed scalar query-feature path.

Reject:

```text
empty property name
empty property value
duplicate property name
collision between a query feature and request rank property with the same effective rank-property key
```

This is a low-level Vespa-facing API. The high-level C/I layers may expose a narrower policy later.

## Native rank features

The verifier freezes a profile using:

```text
nativeRank(title)
```

and inspects:

```text
nativeRank(title)
nativeFieldMatch(title)
nativeProximity(title)
```

Search Kernel must return the values produced by Vespa.

## Feature inspection

M10 v1 uses Vespa match features as the low-level inspection surface.

A rank profile declares match features. For every returned hit, `Hit::match_features` contains the scalar values returned by the native matcher.

The verifier compares exact IEEE-754 bit patterns for:

```text
hit score
match-feature values
```

Feature order is not semantic. Feature names are semantic.

If Vespa returns a non-scalar/data/tensor match feature through this M10 v1 surface, Search Kernel must fail explicitly rather than silently coerce it to a float.

## Ranking expressions

The verifier includes an arithmetic first-phase expression. This proves the expression is compiled/executed by Vespa rather than by a Rust helper.

Search Kernel may perform bounded-string and obvious structural validation. It must not implement a ranking-expression interpreter.

## Required native path

Acceptance must exercise:

```text
Rust RankProfile / SearchRequest
      ↓
Search Kernel native ABI
      ↓
SearchRequest.ranking
SearchRequest.propertiesMap rank properties
      ↓
Proton Matcher / Vespa FEF ranking setup
      ↓
SearchReply::Hit.metric
SearchReply::match_features
```

The pinned Vespa request object owns the selected ranking name and property maps, and the native reply exposes match-feature values. M10 must use those mechanisms rather than a parallel evaluator.

## Configuration path

Rank-profile configuration is compiled into the local Proton config consumed by the embedded engine.

At minimum the emitted rank-profile config must represent:

```text
vespa.rank.firstphase
vespa.match.feature
```

The verifier's direct C++ oracle uses a frozen independent `rank-profiles.cfg`; it does not call the production `ConfigCompiler`.

## Explicit non-goals for M10 v1

Not required:

```text
second-phase ranking
global-phase ranking
query tensors
match-feature tensors
ONNX/model evaluation
ranking constants
model/file distribution
Vespa Java query profiles
search chains
RankScript
high-level C-series Ranking semantics
```

If constants/models are exposed later, embedded operation must use explicit local resources. Search Kernel must not reproduce Vespa file distribution.

## Forbidden shortcuts

The following do not satisfy M10:

```text
Rust BM25 implementation
Rust ranking-expression evaluator
Rust-side score adjustment after native search
reading document attributes in Rust and manufacturing the score
hard-coded fixture scores
hard-coded fixture feature values
calling a verifier oracle from production code
using RankScript as the M10 evaluator
returning requested feature names with invented/default zero values
```

## Positive verifier coverage

The frozen corpus contains exactly eight scenarios:

```text
bm25-title
field-specific-bm25
attribute-ranking-expression
query-feature-controls-ranking
raw-rank-property-equivalence
arithmetic-ranking-expression
native-rank-features
multi-feature-inspection
```

Together they cover every M10 v1 target.

The verifier also runs two focused engine-open configuration regressions outside
the eight-result corpus:

```text
attribute-only schema + one custom rank profile
text schema + custom profile named "bm25"
```

The latter confirms that `bm25` remains legal as a custom profile name; only
`default` is reserved.
