# C2 — Search Kernel IR

This document records milestone history. The final architectural names are
`*Ir` for owned Search Kernel IR types, `IndexContext` for compiled schema
context, and `*_to_ir` for API conversion. Legacy names in older examples are
not part of the final architecture.

Implement:

```text
searchkernel-core
```

`searchkernel-core` performs API → IR conversion from the Portable API's
FlatBuffers contract into the canonical Search Kernel IR.

It owns:

```text
stable defaults
schema validation
request normalization
field capabilities
field cardinality
analyzer identities
value validation
document validation
patch semantics
filter validation
sort validation
facet validation
projection validation
highlight validation
ranking policies
language hints
prefix/typo policy
portable errors
explicit IR requests
backend contract
IR introspection
```

It does **not** implement search.

It does **not** know about Vespa.

---

# 1. Architecture

The final architecture is:

```text
Portable API
        │
        ▼
FlatBuffers contract
        │
        ▼
API → IR conversion
searchkernel-core
*_to_ir
        │
        ▼
Search Kernel IR
owned backend-independent Rust model
        │
        ▼
Backend interface
        │
        ▼
Vespa lowering
lower_*
        │
        ▼
Vespa model / runtime
```

The architectural rule is:

> The portable API describes what the caller expressed. The Search Kernel IR describes the fully validated and explicit meaning of that request. Backend lowering describes how that meaning is represented by the execution engine.

The core invariant is:

> Once an operation crosses the `Backend` boundary, no Search Kernel meaning decision remains to be made.

The backend decides **how** to execute an operation.

It does not decide **what the operation means**.

---

# 2. Dependency firewall

Allowed:

```text
searchkernel-core
    ↓
searchkernel-api
```

Forbidden:

```text
searchkernel-core
    X
searchkernel-vespa

searchkernel-core
    X
searchkernel-vespa-sys

searchkernel-core
    X
searchkernel-vespa-backend
```

No Vespa type or term may appear in the C2 public API.

---

# 3. Portable API and Search Kernel IR

The FlatBuffers contract is the Portable API's language-neutral input/output
contract.

Search Kernel IR types are the fully validated and explicit backend-independent
Rust representation.

For example:

```text
FlatBuffers contract

FieldDefinition {
    name: "title",
    kind: TEXT,
    cardinality: UNSPECIFIED,
    stored: absent,
    searchable: absent,
    ...
}
```

becomes:

```rust
ResolvedField {
    name: FieldName("title"),
    kind: FieldKind::Text,
    cardinality: FieldCardinality::Single,
    analyzer: Some(AnalyzerIdentity::StandardV1),
    capabilities: FieldCapabilities {
        stored: true,
        searchable: true,
        filterable: false,
        sortable: false,
        facetable: false,
    },
}
```

`ResolvedField` is an implementation name for a Search Kernel IR value.

It is not part of the FlatBuffers contract.

---

# 4. C1 presence is semantic

The updated C1 contract deliberately uses wrapper messages where absent and present-empty differ.

C2 must preserve and resolve these distinctions.

```text
IndexDefinition.default_search_fields

absent
    -> derive stable default search fields

present + empty
    -> explicitly no default search fields

present + non-empty
    -> use supplied search fields
```

```text
SearchRequest.search_fields

absent
    -> use index default search fields

present + empty
    -> explicitly search no fields

present + non-empty
    -> use supplied search fields
```

```text
GetRequest.projection
SearchRequest.projection

absent
    -> all stored fields

present + empty
    -> no stored fields

present + non-empty
    -> exactly the supplied fields
```

```text
SearchRequest.highlighting

absent
    -> highlighting disabled

present + empty
    -> explicit highlighting request selecting zero fields

present + non-empty
    -> highlight supplied fields
```

C2 must never collapse these cases before their semantics have been resolved.

---

# 5. Core name types

```rust
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldName(String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexName(String);
```

## FieldName

User field names must match:

```text
[A-Za-z][A-Za-z0-9_]*
```

Names beginning with `_` are reserved by Search Kernel.

Therefore user schemas cannot declare:

```text
_id
_score
_foo
```

## DocumentId

A document ID must be a non-empty UTF-8 string.

It is otherwise opaque.

C2 does not trim, normalize, parse, or reinterpret it.

## IndexName

A present index name must be non-empty.

It is logical metadata.

It is not a path or runtime handle.

---

# 6. Field kinds

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Keyword,
    Int64,
    Float64,
    Bool,
}
```

Mapping:

```text
FIELD_KIND_TEXT     -> Text
FIELD_KIND_KEYWORD  -> Keyword
FIELD_KIND_INT64    -> Int64
FIELD_KIND_FLOAT64  -> Float64
FIELD_KIND_BOOL     -> Bool
```

`FIELD_KIND_UNSPECIFIED` is invalid.

---

# 7. Cardinality

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldCardinality {
    Single,
    Multi,
}
```

Resolution:

```text
FIELD_CARDINALITY_UNSPECIFIED
    -> Single
```

After C2, cardinality is always explicit.

---

# 8. Analyzer identity

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalyzerIdentity {
    StandardV1,
}
```

`StandardV1` corresponds to:

```text
Analyzer {
    name: "standard",
    version: 1,
    options: {}
}
```

Analyzer identities are Search Kernel semantic identities, not backend names.

An incompatible change to analyzer semantics requires:

```text
StandardV2
```

rather than silently changing `StandardV1`.

## Text fields

Absent analyzer:

```text
-> StandardV1
```

Explicit:

```text
standard / version 1 / no options
```

also resolves to:

```text
StandardV1
```

Unsupported name, version, or options return `UNSUPPORTED`.

## Non-text fields

An explicit analyzer is invalid.

Resolved analyzer is:

```text
None
```

---

# 9. Field capabilities

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldCapabilities {
    pub stored: bool,
    pub searchable: bool,
    pub filterable: bool,
    pub sortable: bool,
    pub facetable: bool,
}
```

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedField {
    pub name: FieldName,
    pub kind: FieldKind,
    pub cardinality: FieldCardinality,
    pub analyzer: Option<AnalyzerIdentity>,
    pub capabilities: FieldCapabilities,
}
```

No capability remains unspecified after resolution.

---

# 10. Stable field defaults

For single-valued fields:

| Kind | Analyzer | Stored | Searchable | Filterable | Sortable | Facetable |
|---|---|---:|---:|---:|---:|---:|
| Text | StandardV1 | true | true | false | false | false |
| Keyword | none | true | false | true | true | true |
| Int64 | none | true | false | true | true | false |
| Float64 | none | true | false | true | true | false |
| Bool | none | true | false | true | true | true |

For multi-valued fields the same defaults apply except:

```text
sortable = false
```

C2 v1 has no semantic definition for sorting a multi-valued field.

---

# 11. Capability constraints

`Text` supports:

```text
stored
searchable
```

C2 v1 rejects text fields configured as:

```text
filterable
sortable
facetable
```

`Keyword` supports:

```text
stored
filterable
sortable
facetable
```

and rejects:

```text
searchable = true
```

`Int64` and `Float64` support:

```text
stored
filterable
sortable
facetable
```

and reject:

```text
searchable = true
```

`Bool` supports:

```text
stored
filterable
sortable
facetable
```

and rejects:

```text
searchable = true
```

Any multi-valued field rejects:

```text
sortable = true
```

Unsupported explicit combinations are errors.

They are never silently ignored.

---

# 12. Search-field weights

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SearchFieldWeight(f64);
```

A weight must be:

```text
finite
>= 0.0
```

Absent weight:

```text
-> 1.0
```

Explicit zero remains:

```text
0.0
```

It must not be mistaken for an absent value.

Define:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedSearchField {
    pub field: FieldName,
    pub weight: SearchFieldWeight,
}
```

A search field must:

```text
exist
be searchable
occur at most once
```

---

# 13. Resolved index definition

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedIndexDefinition {
    pub name: Option<IndexName>,

    pub fields: Vec<ResolvedField>,

    pub default_search_fields: Vec<ResolvedSearchField>,

    pub default_ranking: ResolvedRankingPolicy,

    pub default_prefix_policy: PrefixPolicy,

    pub default_typo_policy: TypoPolicy,
}
```

Rules:

```text
at least one field
unique field names
valid field kinds
explicit cardinalities
explicit capabilities
explicit analyzer identity where applicable
```

Field declaration order is preserved.

---

# 14. Default search-field presence

The updated protobuf makes this distinction explicit.

## Absent

```text
default_search_fields absent
```

resolves to:

```text
all searchable schema fields
in schema declaration order
weight = 1.0
```

## Present-empty

```text
default_search_fields present
default_search_fields.fields = []
```

resolves to:

```text
[]
```

This is valid.

It means:

> This index deliberately has no default search fields.

A caller may still explicitly select searchable fields in a request.

## Present-non-empty

Validate the supplied selection and preserve its order.

---

# 15. Scalar values

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedScalarValue {
    Bool(bool),
    Int64(i64),
    Float64(f64),
    String(String),
}
```

Field mapping:

```text
Text     -> String
Keyword  -> String
Int64    -> Int64
Float64  -> Float64
Bool     -> Bool
```

C2 performs no cross-type coercion.

In particular:

```text
int64 -> float64     no
float64 -> int64     no
uint64 -> int64      no
string -> number     no
number -> string     no
```

---

# 16. Unsupported generic Value forms

C1's generic `Value` also supports:

```text
null
uint64
bytes
object
list
```

Those types make C1 extensible.

They do not imply matching C2 field kinds.

For a single-valued v1 field:

```text
null
uint64
bytes
object
list
```

are invalid.

`list` has special meaning only for `Multi` cardinality.

A `Value` with no selected `oneof` is invalid.

---

# 17. Floating-point validity

Every C2 numeric floating-point value must be finite.

Reject:

```text
NaN
+Infinity
-Infinity
```

This applies to:

```text
field values
search-field weights
filter values
range bounds
```

---

# 18. Multi-valued fields

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedFieldValue {
    Single(ResolvedScalarValue),
    Multi(Vec<ResolvedScalarValue>),
}
```

A `Single` field requires a scalar C1 value.

A `Multi` field requires:

```text
Value.list_value
```

Every element must match the field's scalar kind.

C2 does not automatically convert:

```text
scalar -> single-element list
list -> scalar
```

Nested:

```text
lists
objects
null
bytes
```

are invalid.

An empty list is valid.

Duplicates are preserved.

---

# 19. Multi-value query semantics

Filtering a multi-valued field has explicit Search Kernel semantics.

```text
Eq(v)
    true if any element equals v
```

```text
Ne(v)
    true if no element equals v
```

```text
Lt/Lte/Gt/Gte(v)
    true if any element satisfies the comparison
```

```text
Range(...)
    true if at least one individual element satisfies
    the complete range predicate
```

```text
Exists
    true if the list contains at least one value
```

For facets:

> One document contributes at most one count to a particular facet value, even if that value occurs multiple times in the field.

For searchable multi-valued text:

> Each list value is a distinct value of the same field. Phrase matching must not cross value boundaries.

---

# 20. Resolved document

```rust
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedDocument {
    pub id: DocumentId,
    pub fields: BTreeMap<FieldName, ResolvedFieldValue>,
}
```

Resolution validates:

```text
document present
non-empty document id
known field names
field cardinality
field value type
finite numbers
```

A missing field is absent.

Explicit `null` is not an absent field.

Explicit `null` is invalid in C2 v1.

---

# 21. Insert

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedInsertRequest {
    pub document: ResolvedDocument,
}
```

Semantics:

```text
missing id
    -> insert

existing id
    -> ALREADY_EXISTS
```

Existence is checked by the backend.

Success returns C1:

```text
InsertResult { id }
```

---

# 22. Upsert

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedUpsertRequest {
    pub document: ResolvedDocument,
}
```

Upsert is **full-document replacement**.

```text
missing id
    -> create

existing id
    -> replace all user fields
```

Fields not present in the incoming document are absent after the operation.

Success returns:

```text
UpsertResult { id }
```

---

# 23. Patch

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedPatchOperation {
    Set {
        field: FieldName,
        value: ResolvedFieldValue,
    },

    Unset {
        field: FieldName,
    },
}
```

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPatchRequest {
    pub id: DocumentId,
    pub operations: Vec<ResolvedPatchOperation>,
}
```

Rules:

```text
non-empty id
at least one operation
operation oneof must be set
field must exist
Set.value must exist
Set value must match kind/cardinality
same field may occur at most once
```

This is invalid:

```text
Set price
Unset price
```

within the same patch.

`Unset` on an already-absent field is harmless.

Patching a nonexistent document returns:

```text
NOT_FOUND
```

Success returns:

```text
PatchResult { id }
```

---

# 24. Delete

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedDeleteRequest {
    pub id: DocumentId,
}
```

Delete is idempotent.

C1 result semantics are:

```text
deleted = true
    -> document existed and was deleted

deleted = false
    -> document did not exist
```

A missing document is not an error.

---

# 25. Projection

Projection presence is now explicit in C1.

C2 resolves projection all the way to the actual field list:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedProjection {
    pub fields: Vec<FieldName>,
}
```

The backend never decides what "default projection" means.

## Absent projection

Resolve to:

```text
every stored field
in schema declaration order
```

## Present-empty projection

Resolve to:

```text
[]
```

This means:

> Return the document ID/result metadata but no stored document fields.

## Present-non-empty projection

Every field must:

```text
exist
be stored
occur at most once
```

Caller order is preserved.

---

# 26. Get

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedGetRequest {
    pub id: DocumentId,
    pub projection: ResolvedProjection,
}
```

`GetRequest.projection` therefore resolves as:

```text
absent
    -> all stored fields

present empty
    -> zero fields

present non-empty
    -> validated explicit fields
```

A missing document is represented by the absence of:

```text
GetResult.document
```

and is not an error.

---

# 27. Comparison operators

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparisonOp {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
}
```

`COMPARISON_OP_UNSPECIFIED` is invalid.

---

# 28. Resolved comparison

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedComparison {
    pub field: FieldName,
    pub op: ComparisonOp,
    pub value: ResolvedScalarValue,
}
```

Comparison values are scalar even when the target field is multi-valued.

---

# 29. Range filters

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedRange {
    pub field: FieldName,

    pub lower: Option<ResolvedScalarValue>,
    pub upper: Option<ResolvedScalarValue>,

    pub include_lower: bool,
    pub include_upper: bool,
}
```

Defaults:

```text
include_lower absent -> true
include_upper absent -> true
```

At least one bound must exist.

If a bound is absent, its corresponding `include_*` flag must also be absent.

Both bounds must match the field type.

For numeric ranges:

```text
lower > upper
```

is invalid.

Equal bounds with an exclusive endpoint are valid and represent an empty range.

---

# 30. Resolved filters

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedFilterExpr {
    And(Vec<ResolvedFilterExpr>),

    Or(Vec<ResolvedFilterExpr>),

    Not(Box<ResolvedFilterExpr>),

    Comparison(ResolvedComparison),

    Range(ResolvedRange),

    Exists {
        field: FieldName,
    },
}
```

C1 mapping:

```text
FilterExpr.all      -> And
FilterExpr.any      -> Or
FilterExpr.negation -> Not
```

A `FilterExpr` with no selected `oneof` is invalid.

---

# 31. Filter capabilities

Every referenced field must have:

```text
filterable = true
```

Allowed operations:

| Kind | Eq / Ne | Lt / Lte / Gt / Gte | Range |
|---|---:|---:|---:|
| Keyword | yes | no | no |
| Int64 | yes | yes | yes |
| Float64 | yes | yes | yes |
| Bool | yes | no | no |
| Text | no | no | no |

`Exists` is valid on any filterable field.

---

# 32. Filter normalization

C2 performs safe structural normalization:

```text
And([A, And([B, C])])
    -> And([A, B, C])
```

```text
Or([A, Or([B, C])])
    -> Or([A, B, C])
```

```text
And([A])
    -> A

Or([A])
    -> A
```

Invalid:

```text
And([])
Or([])
Not without child
```

Child ordering is preserved.

C2 does not reorder predicates for optimization.

---

# 33. Sorting

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingPlacement {
    First,
    Last,
}
```

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedSortExpr {
    pub field: FieldName,
    pub direction: SortDirection,
    pub missing: MissingPlacement,
}
```

Rules:

```text
field exists
sortable = true
cardinality = Single
same field appears at most once
```

Defaults:

```text
SORT_DIRECTION_UNSPECIFIED
    -> Asc
```

```text
missing absent
    -> Last
```

Because `missing` has presence:

```text
missing present with MISSING_PLACEMENT_UNSPECIFIED
    -> invalid
```

Explicit sort order is preserved.

C2 adds no hidden tie-break sort.

---

# 34. Facets

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FacetSort {
    CountDesc,
    CountAsc,
    ValueAsc,
    ValueDesc,
}
```

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedFacetRequest {
    pub field: FieldName,
    pub limit: u32,
    pub sort: FacetSort,
}
```

Rules:

```text
field exists
facetable = true
same field appears at most once
```

Defaults:

```text
limit absent
    -> 10

sort absent
    -> CountDesc
```

Because facet sort has presence:

```text
sort present with FACET_SORT_UNSPECIFIED
    -> invalid
```

Explicit:

```text
limit = 0
```

is valid and requests zero facet values.

---

# 35. Facet value ordering

`ValueAsc` / `ValueDesc` use the field's semantic scalar ordering:

```text
Keyword
    -> lexicographic Unicode scalar-value ordering

Int64
    -> signed integer ordering

Float64
    -> numeric ordering

Bool
    -> false < true
```

All float values are finite.

Text fields cannot be facetable in C2 v1.

---

# 36. Ranking policy

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedRankingPolicy {
    DefaultRelevanceV1,
    Bm25,
}
```

Resolution:

```text
SearchRequest.ranking absent
    -> DefaultRelevanceV1
```

Explicit policies map directly.

A present `RankingPolicy` whose `oneof` is unset is invalid.

C2 defines ranking policy identity.

It does not execute ranking.

No Vespa rank-profile or ranking-expression concept appears here.

---

# 37. Prefix policy

C1 currently contains no configurable prefix-search field.

C2 nevertheless owns this semantic dimension.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefixPolicy {
    Disabled,
}
```

C2 v1 index default:

```text
Disabled
```

Every search therefore resolves to an explicit:

```text
PrefixPolicy::Disabled
```

A backend may not independently enable prefix search.

Future prefix semantics require a compatible C1 extension and new versioned C2 variants.

---

# 38. Typo policy

Likewise:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypoPolicy {
    Disabled,
}
```

C2 v1 default:

```text
Disabled
```

A backend may not independently enable typo/fuzzy matching.

Future behavior requires an explicit C1 extension.

---

# 39. Query text

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedQuery {
    MatchAll,
    Text(String),
}
```

C1 has no query presence bit, so C2 defines:

```text
query empty
    -> MatchAll

query all whitespace
    -> MatchAll

otherwise
    -> Text(original string)
```

For a non-empty query, the original string is preserved exactly.

C2 does not tokenize or analyze it.

---

# 40. Per-request search-field selection

The updated C1 wrapper gives three distinct states.

## Absent

```text
SearchRequest.search_fields absent
```

resolves to:

```text
ResolvedIndexDefinition.default_search_fields
```

## Present-empty

```text
SearchRequest.search_fields present
fields = []
```

resolves to:

```text
[]
```

It does **not** fall back to index defaults.

## Present-non-empty

Resolve and validate exactly the supplied fields.

Request-specific search fields replace index defaults.

They do not merge with them.

---

# 41. Search fields and query validity

For:

```text
ResolvedQuery::Text(...)
```

the effective search-field list must contain at least one field.

Otherwise return:

```text
FAILED_PRECONDITION
detail code = "no_searchable_fields"
```

For:

```text
ResolvedQuery::MatchAll
```

an empty search-field selection is valid.

---

# 42. Highlighting

Presence must remain distinguishable.

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedHighlighting {
    Disabled,

    Requested {
        fields: Vec<FieldName>,
    },
}
```

## Absent

```text
SearchRequest.highlighting absent
```

becomes:

```text
ResolvedHighlighting::Disabled
```

## Present-empty

```text
SearchRequest.highlighting present
highlighting.fields = []
```

becomes:

```text
ResolvedHighlighting::Requested {
    fields: vec![],
}
```

This is valid.

In C2 v1 it selects no fields, but its presence is preserved so future highlighting options can be added without redefining absence semantics.

## Present-non-empty

Each field must:

```text
exist
kind = Text
searchable = true
stored = true
occur at most once
```

---

# 43. Language hints

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageHint(String);
```

A present language must be non-empty after validation.

C2 v1 accepts basic BCP-47-style ASCII tags:

```text
[A-Za-z]{2,8}(-[A-Za-z0-9]{1,8})*
```

Canonical form is lowercase ASCII.

Examples:

```text
EN
    -> en

en-US
    -> en-us
```

C2 chooses and normalizes the language hint.

It does not execute linguistic analysis.

---

# 44. Resolved search request

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedSearchRequest {
    pub query: ResolvedQuery,

    pub search_fields: Vec<ResolvedSearchField>,

    pub filter: Option<ResolvedFilterExpr>,

    pub sort: Vec<ResolvedSortExpr>,

    pub facets: Vec<ResolvedFacetRequest>,

    pub ranking: ResolvedRankingPolicy,

    pub limit: u32,

    pub offset: u32,

    pub projection: ResolvedProjection,

    pub highlighting: ResolvedHighlighting,

    pub language: Option<LanguageHint>,

    pub prefix_policy: PrefixPolicy,

    pub typo_policy: TypoPolicy,
}
```

Nothing in this object is unresolved.

---

# 45. Search defaults

C2 v1 freezes:

```text
query empty/whitespace
    -> MatchAll

search_fields absent
    -> index.default_search_fields

filter absent
    -> None

sort empty
    -> []

facets empty
    -> []

ranking absent
    -> DefaultRelevanceV1

limit absent
    -> 20

offset absent
    -> 0

projection absent
    -> all stored fields

highlighting absent
    -> Disabled

language absent
    -> None

prefix
    -> Disabled

typo
    -> Disabled
```

Explicit:

```text
limit = 0
```

is valid.

C2 does not impose an arbitrary maximum result limit.

Backend implementation limits must produce an explicit error rather than silently clamping the request.

---

# 46. Analyze

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedAnalyzeRequest {
    pub analyzer: AnalyzerIdentity,
    pub text: String,
    pub language: Option<LanguageHint>,
}
```

C1 allows two targets.

## Explicit analyzer

Resolve the supplied `Analyzer`.

## Field

The field must:

```text
exist
kind = Text
have a resolved analyzer
```

The field's resolved analyzer becomes the request analyzer.

A missing `target` oneof is invalid.

Empty analysis text is valid and may yield zero tokens.

C2 selects the analyzer.

Actual analysis is a backend operation.

---

# 47. Describe

`DescribeRequest` is answered entirely by C2.

It does not call `Backend`.

```rust
impl IndexSemantics {
    pub fn describe(&self) -> api::DescribeResult;
}
```

The returned `IndexDefinition` is an effective, fully materialized description.

Each field has explicit:

```text
kind
cardinality
stored
searchable
filterable
sortable
facetable
analyzer where applicable
```

`default_search_fields` is always present in the result and contains the effective resolved selection.

Therefore:

```text
input default_search_fields absent
```

may describe as:

```text
present {
    fields: all derived searchable fields
}
```

while:

```text
input default_search_fields present-empty
```

describes as:

```text
present {
    fields: []
}
```

No backend configuration is exposed.

---

# 48. InspectQuery

`InspectQueryRequest` is also answered entirely by C2.

```rust
impl IndexSemantics {
    pub fn inspect_query(
        &self,
        request: &api::InspectQueryRequest,
    ) -> Result<api::InspectQueryResult, PortableError>;
}
```

It:

```text
takes request.request
converts it through the normal API → IR path
converts the Search Kernel IR back to the FlatBuffers contract
returns it as the normalized request
```

It does not invoke `Backend`.

---

# 49. InspectQuery normalized representation

The returned `SearchRequest` materializes all C1-representable resolved semantics.

It must contain:

```text
effective search_fields wrapper
explicit search-field weights

normalized filter tree

explicit sort directions
explicit missing placement

explicit facet limits
explicit facet sort

explicit ranking

explicit limit
explicit offset

explicit projection wrapper containing
the actual effective field list

normalized language
```

Query representation:

```text
MatchAll
    -> query = ""

Text(s)
    -> query = s
```

Highlighting:

```text
Disabled
    -> highlighting absent

Requested { fields }
    -> highlighting present with exactly those fields
```

Thus explicit present-empty highlighting survives inspection as present-empty.

Prefix/typo are not C1 fields.

They are fixed to `Disabled` in C2 v1, so there is no request-dependent information to serialize.

---

# 50. Portable errors

C2 maps exactly onto C1's portable error model.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidArgument,
    NotFound,
    AlreadyExists,
    FailedPrecondition,
    Conflict,
    Unsupported,
    ResourceExhausted,
    Internal,
    Unavailable,
}
```

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorDetail {
    pub code: String,
    pub path: Option<String>,
    pub message: String,
    pub metadata: std::collections::BTreeMap<String, String>,
}
```

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortableError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Vec<ErrorDetail>,
    pub retryable: bool,
}
```

Conversion between these and C1 `Error` must be lossless.

---

# 51. Stable semantic detail codes

C2 should use stable detail codes including:

```text
invalid_field_name
duplicate_field
unknown_field

invalid_field_kind
invalid_cardinality

invalid_analyzer
unsupported_analyzer

invalid_capability
field_not_searchable
field_not_filterable
field_not_sortable
field_not_facetable
field_not_stored

type_mismatch
cardinality_mismatch
non_finite_number
missing_value

invalid_document_id

empty_patch
invalid_patch_operation
duplicate_patch_field

invalid_filter
empty_boolean_filter
invalid_range

duplicate_search_field
invalid_search_weight
no_searchable_fields

duplicate_sort_field
duplicate_facet_field

duplicate_projection_field
duplicate_highlight_field

invalid_language
```

Applications may branch on:

```text
Error.code
ErrorDetail.code
ErrorDetail.path
```

They must not branch on human-readable prose.

---

# 52. Canonical error paths

Paths follow the updated C1 shape.

Examples:

```text
fields[2].name
fields[2].analyzer

default_search_fields.fields[0].field
default_search_fields.fields[0].weight

document.id
document.fields.price

operations[1].set.field
operations[1].set.value

search_fields.fields[0].field
search_fields.fields[0].weight

filter.all.clauses[1].comparison.field
filter.any.clauses[0]
filter.negation.clause
filter.range.lower

sort[0].field
facets[1].field

projection.fields[0]
highlighting.fields[0]

ranking
language
```

These names intentionally follow the new protobuf names:

```text
all
any
negation
projection
highlighting
```

---

# 53. Error classes

Semantic input validation normally returns:

```text
INVALID_ARGUMENT
```

Examples:

```text
unknown field
wrong value type
bad range
duplicate sort field
invalid explicit projection
```

A structurally valid request that cannot execute because of the resolved index semantics returns:

```text
FAILED_PRECONDITION
```

Example:

```text
text query with zero effective search fields
```

An explicitly requested semantic feature or analyzer unsupported by C2 returns:

```text
UNSUPPORTED
```

C2-generated errors have:

```text
retryable = false
```

---

# 54. Index semantic context

```rust
pub struct IndexSemantics {
    definition: ResolvedIndexDefinition,
}
```

Construction:

```rust
impl IndexSemantics {
    pub fn new(
        definition: &api::IndexDefinition,
    ) -> Result<Self, PortableError>;
}
```

Also expose:

```rust
pub fn resolve_index_definition(
    definition: &api::IndexDefinition,
) -> Result<ResolvedIndexDefinition, PortableError>;
```

---

# 55. Resolution API

```rust
impl IndexSemantics {
    pub fn definition(
        &self,
    ) -> &ResolvedIndexDefinition;

    pub fn resolve_insert(
        &self,
        request: &api::InsertRequest,
    ) -> Result<ResolvedInsertRequest, PortableError>;

    pub fn resolve_upsert(
        &self,
        request: &api::UpsertRequest,
    ) -> Result<ResolvedUpsertRequest, PortableError>;

    pub fn resolve_patch(
        &self,
        request: &api::PatchRequest,
    ) -> Result<ResolvedPatchRequest, PortableError>;

    pub fn resolve_delete(
        &self,
        request: &api::DeleteRequest,
    ) -> Result<ResolvedDeleteRequest, PortableError>;

    pub fn resolve_get(
        &self,
        request: &api::GetRequest,
    ) -> Result<ResolvedGetRequest, PortableError>;

    pub fn resolve_search(
        &self,
        request: &api::SearchRequest,
    ) -> Result<ResolvedSearchRequest, PortableError>;

    pub fn resolve_analyze(
        &self,
        request: &api::AnalyzeRequest,
    ) -> Result<ResolvedAnalyzeRequest, PortableError>;

    pub fn describe(
        &self,
    ) -> api::DescribeResult;

    pub fn inspect_query(
        &self,
        request: &api::InspectQueryRequest,
    ) -> Result<api::InspectQueryResult, PortableError>;
}
```

---

# 56. Backend-bound operations

Only operations requiring engine execution cross the backend boundary.

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedOperation {
    Insert(ResolvedInsertRequest),
    Upsert(ResolvedUpsertRequest),
    Patch(ResolvedPatchRequest),
    Delete(ResolvedDeleteRequest),
    Get(ResolvedGetRequest),
    Search(ResolvedSearchRequest),
    Analyze(ResolvedAnalyzeRequest),
}
```

This is an internal Rust semantic enum.

It is **not** a transport envelope.

C1 deliberately no longer defines:

```text
ApiRequest
ApiResponse
```

Transport framing belongs to C5.

---

# 57. Backend trait

```rust
pub trait Backend: Send + Sync {
    fn insert(
        &self,
        request: ResolvedInsertRequest,
    ) -> Result<api::InsertResult, PortableError>;

    fn upsert(
        &self,
        request: ResolvedUpsertRequest,
    ) -> Result<api::UpsertResult, PortableError>;

    fn patch(
        &self,
        request: ResolvedPatchRequest,
    ) -> Result<api::PatchResult, PortableError>;

    fn delete(
        &self,
        request: ResolvedDeleteRequest,
    ) -> Result<api::DeleteResult, PortableError>;

    fn get(
        &self,
        request: ResolvedGetRequest,
    ) -> Result<api::GetResult, PortableError>;

    fn search(
        &self,
        request: ResolvedSearchRequest,
    ) -> Result<api::SearchResult, PortableError>;

    fn analyze(
        &self,
        request: ResolvedAnalyzeRequest,
    ) -> Result<api::AnalyzeResult, PortableError>;
}
```

`Describe` and `InspectQuery` are intentionally absent.

They are C2-local semantic operations.

---

# 58. Backend responsibilities

The backend performs:

```text
document existence checks
actual mutations
get
linguistic analysis
query compilation
matching
filter execution
sorting
ranking
faceting
highlight generation
persistence
```

The backend does **not**:

```text
choose Search Kernel defaults
interpret absent wrapper messages
interpret present-empty wrapper messages
validate field capabilities
decide analyzer identity
interpret cardinality
type-check values
normalize filters
choose ranking policy
choose projection defaults
normalize language
enable prefix search
enable typo tolerance
```

---

# 59. No bulk operation in C2

The current C1 contract does not define bulk mutation.

Therefore C2 must not invent:

```text
BulkWriteRequest
BulkWriteResult
ResolvedBulkWriteRequest
```

as public semantics.

Before bulk becomes part of the product API, C1 must first be compatibly extended.

Only then should C2 gain a resolved bulk representation.

---

# 60. No C1 transport envelope

The updated protobuf deliberately states:

```text
Transport framing deliberately does not live in C1.
```

C2 must respect that boundary.

It must not recreate a protobuf:

```text
ApiRequest
ApiResponse
```

C5 owns the native ABI lifecycle and operation framing once that design is frozen.

`ResolvedOperation` exists only as convenient Rust semantic/test infrastructure.

---

# 61. Deterministic canonicalization

Search Kernel IR output must be deterministic.

```text
schema fields
    preserve declaration order

derived default search fields
    schema declaration order

explicit default search fields
    caller order

request search fields
    caller order

document fields
    canonical FieldName ordering

projection fields
    schema order when derived
    caller order when explicit

highlight fields
    caller order

sort
    caller order

facets
    caller order

patch operations
    caller order

And / Or
    flatten while preserving child order
```

Never depend on protobuf map iteration order.

---

# 62. Conformance serialization

Search Kernel IR Rust types may use deterministic `serde` serialization for
golden tests.

For example:

```text
ResolvedSearchRequest
    -> canonical JSON
```

This is a C2 conformance format only.

It is **not** a public protocol.

The FlatBuffers contract remains the interchange format.

Conformance must compare semantic values, not protobuf byte serialization order.

---

# 63. What C2 does not implement

C2 must not implement:

```text
tokenization
stemming
linguistic analysis

query-tree construction
term matching
phrase matching

BM25 computation
ranking execution

filter execution
sort execution
facet counting
highlight generation

prefix matching
fuzzy matching

persistence
storage
transactions

Vespa configuration
Vespa query objects
Vespa ranking expressions

native ABI framing
```

---

# 64. Relationship to C4

C4 owns ergonomic Rust syntax such as:

```rust
Field::text("title")
```

That may compile into a C1:

```text
FieldDefinition {
    name: "title"
    kind: TEXT
    cardinality: UNSPECIFIED

    stored: absent
    searchable: absent
    filterable: absent
    sortable: absent
    facetable: absent

    analyzer: absent
}
```

C2 then creates the `ResolvedField`.

The path is:

```text
C4 Rust convenience API

Field::text("title")

        ↓

FlatBuffers contract meaning

FieldDefinition

        ↓

Search Kernel IR

ResolvedField

        ↓

I-series adapter

        ↓

M-series Vespa API
```

---

# 65. Relationship to C3

C3 implements:

```text
RecordingBackend
ScriptedBackend
```

against the C2 `Backend` trait.

`RecordingBackend` records the exact `ResolvedOperation` produced by C2.

It does not search.

`ScriptedBackend` returns predetermined C1 result/error values.

It does not search.

This verifies both directions independently:

```text
public request
    ↓
resolved semantic operation
```

and:

```text
portable result/error
    ↓
public client result/error
```

---

# 66. C2 verifier structure

Create:

```text
reference/clients/c2/
├── CONTRACT.md
├── api-contract/
├── scenarios/
├── expected/
└── verify-c2.sh
```

C2 verification must require no:

```text
Vespa
native SDK
Java
Docker
```

---

# 67. Required schema scenarios

Verify:

```text
Text defaults
Keyword defaults
Int64 defaults
Float64 defaults
Bool defaults

Single cardinality default
explicit Multi

valid capability overrides
invalid capabilities
multi-valued sortable rejection

implicit StandardV1
explicit StandardV1
unsupported analyzer name
unsupported analyzer version
unsupported analyzer options
analyzer on non-text field

duplicate fields
invalid field name
reserved field name
```

Also specifically verify wrapper presence:

```text
default_search_fields absent
    -> derived fields

default_search_fields present-empty
    -> []

default_search_fields present-non-empty
    -> explicit fields
```

And:

```text
default weight absent
explicit zero weight
negative weight
NaN weight
Infinity weight
duplicate search field
unknown search field
non-searchable search field
```

---

# 68. Required document and patch scenarios

Verify:

```text
valid scalar fields
valid mixed document

valid multi field
valid empty multi list

single/list mismatch
multi/scalar mismatch

wrong scalar type
null rejection
uint64 rejection
bytes rejection
object rejection
nested list rejection

NaN rejection
Infinity rejection

unknown field
empty id

Set
Unset
Set multi field

empty patch
operation oneof absent
Set value absent
duplicate patch field
Set + Unset same field
```

---

# 69. Required filter scenarios

Verify:

```text
Keyword Eq / Ne

Int64 comparisons
Float64 comparisons
numeric ranges

Bool Eq / Ne

multi-value comparison semantics
multi-value range semantics
multi-value Exists semantics

unknown field
non-filterable field

Keyword range rejection
Bool range rejection

lower-only range
upper-only range
inclusive defaults
exclusive bounds
no bounds
lower > upper

all
any
negation

nested all flattening
nested any flattening

empty all
empty any
missing negation clause
missing FilterExpr oneof
unspecified ComparisonOp
```

---

# 70. Required sort/facet scenarios

Sort:

```text
unspecified direction -> Asc
explicit Asc
explicit Desc

missing absent -> Last
explicit First
explicit Last
explicit UNSPECIFIED rejection

unknown field
non-sortable field
multi-valued sort rejection
duplicate sort field
```

Facet:

```text
Keyword facet
Bool facet
explicit numeric facet

limit absent -> 10
limit = 0
explicit nonzero limit

sort absent -> CountDesc
CountDesc
CountAsc
ValueAsc
ValueDesc
explicit UNSPECIFIED rejection

non-facetable field
duplicate facet field

multi-valued facet
duplicate values count once per document
```

---

# 71. Required projection scenarios

For both `GetRequest` and `SearchRequest`:

```text
projection absent
    -> all stored fields

projection present-empty
    -> []

projection present-non-empty
    -> exactly selected fields
```

Also verify:

```text
unknown field
non-stored field
duplicate field
ordering preservation
```

---

# 72. Required highlighting scenarios

Verify:

```text
highlighting absent
    -> Disabled

highlighting present-empty
    -> Requested { fields: [] }

highlighting present with valid text fields
    -> Requested { ... }

unknown field
non-text field
non-searchable text
non-stored text
duplicate field
```

Critically, verify that:

```text
absent
```

and:

```text
present-empty
```

remain distinct through `InspectQuery`.

---

# 73. Required search scenarios

Verify:

```text
empty query -> MatchAll
whitespace query -> MatchAll
non-empty query -> Text(original)
```

Search-field presence:

```text
request search_fields absent
    -> index defaults

request search_fields present-empty
    -> []

request search_fields present-non-empty
    -> request selection
```

Then verify:

```text
Text query + zero effective search fields
    -> FAILED_PRECONDITION

MatchAll + zero effective search fields
    -> valid
```

Also verify:

```text
default ranking
explicit DefaultRelevanceV1
explicit BM25
present RankingPolicy with no oneof

default limit = 20
explicit limit = 0

default offset = 0
explicit offset

language absent
language normalization
invalid language

prefix = Disabled
typo = Disabled
```

---

# 74. Required introspection scenarios

`Describe` must verify:

```text
field defaults materialized
cardinality materialized
analyzer materialized
capabilities materialized
effective default search fields materialized
effective weights materialized
```

`InspectQuery` must verify:

```text
effective search_fields wrapper present
weights explicit

filter normalized

sort defaults explicit

facet defaults explicit

ranking explicit

limit/offset explicit

projection wrapper present
effective projection fields explicit

language normalized

highlighting absent remains absent

highlighting present-empty remains present-empty

highlighting present-non-empty remains present
```

Neither operation may invoke the backend.

---

# 75. Required error tests

For every invalid semantic scenario verify:

```text
ErrorCode
ErrorDetail.code
ErrorDetail.path
retryable
```

Do not require exact human-readable message wording.

Error paths must use the updated protobuf vocabulary:

```text
all
any
negation
projection
highlighting
search_fields.fields
default_search_fields.fields
```

---

# 76. Structural verifier requirements

`verify-c2.sh` must prove:

```text
searchkernel-core depends on searchkernel-api

searchkernel-core does not depend on a Vespa crate

no Vespa crate is reachable through the C2 dependency graph

required Resolved* types compile

IndexSemantics API compiles

Backend trait compiles

a trivial RecordingBackend can implement Backend

Describe requires no Backend

InspectQuery requires no Backend

semantic goldens pass

error goldens pass
```

Also run:

```text
cargo fmt --check

cargo clippy --workspace --all-targets -- -D warnings

cargo test --workspace
```

for the standalone C-series workspace.

---

# 77. Definition of done

C2 is complete only when:

```text
the FlatBuffers contract is the sole Portable API contract

presence semantics are implemented exactly

all defaults are frozen

all fields resolve completely

cardinality is explicit

analyzer identity is explicit

documents are type-checked

patches are normalized

filters are validated and normalized

sorts are explicit

facets are explicit

projections resolve to concrete field lists

highlight presence is preserved

ranking is explicit

language is normalized

prefix policy is explicitly Disabled

typo policy is explicitly Disabled

portable errors map losslessly to C1

Describe is backend-free

InspectQuery is backend-free

C3 can implement Backend without Vespa

no transport envelope is introduced

no bulk API is invented

no Vespa dependency exists
```

The following must all work without linking a search engine:

```text
IndexDefinition
    ↓
ResolvedIndexDefinition
```

```text
InsertRequest
    ↓
ResolvedInsertRequest
```

```text
PatchRequest
    ↓
ResolvedPatchRequest
```

```text
GetRequest
    ↓
ResolvedGetRequest
```

```text
SearchRequest
    ↓
ResolvedSearchRequest
```

```text
AnalyzeRequest
    ↓
ResolvedAnalyzeRequest
```

---

# 78. Fundamental separation

The Portable API answers:

> What did the caller express?

The Search Kernel IR answers:

> What does that mean in Search Kernel?

The backend answers:

> How do I execute that meaning?

C5 later answers:

> How is that operation transported across the native ABI?

These are four separate concerns and must remain separate.
