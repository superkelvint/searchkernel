# M7 — Vespa query compilation

## Goal

Replace the minimum Java query-preparation path required by the embedded engine.

M7 exposes a typed low-level Vespa-facing query representation and a deliberately bounded YQL parser. Both forms must compile through the same field-aware Rust lowering path and execute through the real Vespa Matcher.

```text
Typed Query ─┐
             ├─> field validation ─> M3 text analysis ─> native Vespa query tree ─> Matcher
YQL ─> Query ┘
```

M7 is not the C-series product query API.

## Frozen v1 operators

```text
text:
  term
  phrase

boolean:
  AND
  OR
  NOT

integer attribute:
  equality
  range
  exists
```

M7 intentionally restricts equality/range/exists to M6 `Integer` attribute fields. String, float, and boolean query operators are deferred until the low-level attribute surface is broadened. This avoids pulling M9 work into M7.

## YQL

YQL is a required M7 capability.

The supported YQL subset is the syntax necessary to express the frozen M7 operators:

```text
select * from sources * where <expr>
select * from <document-type> where <expr>

field contains "term"
field contains phrase("a", "b", ...)

expr and expr
expr or expr
!(expr)

integer_field = INTEGER
range(integer_field, LOWER, UPPER)
({bounds:"leftOpen"}range(...))
({bounds:"rightOpen"}range(...))
({bounds:"open"}range(...))
range(integer_field, -Infinity, Infinity)
```

Normal parentheses and Vespa/YQL boolean precedence must be honored. For an
`AND`, canonicalization must place positive children first and negated
children afterward, matching Vespa's `buildAnd()` representation. Multiple
negative siblings are represented as separate `Not(...)` children in the M7
one-child `Not` model.

The following are explicitly outside M7 even though full Vespa YQL can express them:

```text
projection lists other than `*`
sorting
limit / offset
ranking selection
summaries
grouping / pipes
userInput / userQuery
near / onear
fuzzy / prefix / regexp
nearestNeighbor / tensors
weighted sets / dot products
parameter substitution
```

Unsupported YQL constructs must return a deterministic error. They must not be silently accepted and ignored.

### YQL source selectors

An `Engine` owns one compiled Vespa document type. Source selection therefore
has the following M7 semantics:

```text
from sources *       -> the configured document type
from <document-type> -> accepted only when it names the configured type
any other named source -> deterministic unsupported_yql error
```

This preserves the source-set behavior of Vespa's `YqlParser` within M7's
single-document-type boundary. A named source is not merely syntax: it is
validated when the complete YQL statement is searched.

## Typed API semantics

The frozen API contract is `reference/vespa/api-contract/src/bin/m7.rs`.

The semantic shape is:

```text
RangeBound
  Unbounded
  Included(i64)
  Excluded(i64)

Query
  Term { field, text }
  Phrase { field, text }
  And(Vec<Query>)
  Or(Vec<Query>)
  Not(Box<Query>)
  Equals { field, value }
  Range { field, lower, upper }
  Exists { field }
```

`Query::from_yql()` parses only the frozen subset above and returns the same
typed expression representation used by direct callers. `Query` deliberately
does not contain the YQL source selector, so this method is an expression
projection rather than a complete source-aware YQL query. It has no schema
against which to validate a named source.

`Engine::search(SearchRequest)` performs field-aware validation and native compilation.
`Query::from_yql()` parses the complete YQL statement, validates its source
selector against the engine's configured document type, and lowers the
resulting expression through the same path as `search()`. It is
equivalent to `Query::from_yql()` followed by `search()` for the query
expression and for `sources *` (and the configured document type), but the
source-validation step is intentionally retained for named sources.

## Field-aware rules

For schemas compiled by M6:

```text
Term/Phrase:
  field must exist
  field must be M6 Text
  field must be indexed

Equals/Range/Exists:
  field must exist
  field must be M6 Integer
  field must be an attribute
```

Wrong field type and unknown field are errors, not empty result sets.

`And` and `Or` must contain at least one child.

## Linguistics

Text query leaves are raw text at the public `Query` boundary.

M3 analysis is performed when lowering text leaves for execution:

```text
Query::Term/Phrase raw text
       ↓
M3 Linguistics::analyze_query
       ↓
analyzed term(s)
       ↓
Vespa query tree
```

The YQL parser does not implement a second tokenizer/stemmer.

The verifier has cases where documents only match if M3 stemming is actually applied at query time.

## Native compilation

The native representation must use real Vespa query nodes / stack dump consumed by `DocumentDB::match`.

Boolean structure and numeric ranges must be represented in the native query tree. Rust must not fetch broad results and post-filter them.

Phrase matching must continue to use M4 position data.

## NOT semantics

The frozen typed `Not(child)` is logical unary NOT.

For a pure negative query, Vespa semantics are:

```text
TRUE AND NOT child
```

This matches all documents except those matching `child`, consistent with Vespa `NotItem` behavior.

## Exists semantics

M7 `Exists` is defined only for integer attributes.

It lowers to Vespa's full numeric range:

```text
range(field, -Infinity, Infinity)
```

Documents where the attribute is unset do not match.

## Range semantics

Bounds are explicit and preserved:

```text
Unbounded
Included(value)
Excluded(value)
```

YQL `bounds` annotations map to the same representation.

No hit-limit/top-k range semantics are part of M7.

## Verification authority

M7 has three independent authorities:

1. the frozen typed API contract;
2. the pinned Vespa Java `YqlParser` for supported YQL structure/semantics,
   including source selection as bounded by M7's one-document-type index;
3. the real embedded C++ Matcher for execution results.

The Java oracle is verification-only. Production must never invoke Java/JVM/Maven.

## Required scenarios

The frozen scenario corpus covers:

```text
M3 stemming through term query
phrase positions + stemming
AND
OR
pure NOT
nested boolean structure
integer equality
closed range
open range
unbounded range
integer exists
YQL precedence
named YQL source syntax acceptance; source selection/rejection
YQL AND/NOT canonicalization
```

Scenario 13's canonical `Query` expectation intentionally verifies only that
the named-source syntax is accepted and that its expression is lowered
correctly; the source selector cannot appear in the source-free `Query` AST.
Named-source semantics are verified separately by the negative
`unsupported-query-source` case and by the real search path.

Negative scenarios cover:

```text
unknown field
term on integer
numeric equality on text
empty AND/OR
malformed YQL
unsupported ORDER BY
unsupported LIMIT
unsupported projection
unsupported named source
```

## Forbidden shortcuts

Production must not:

```text
invoke Java or Maven
read reference/vespa/m7 scenarios/expected/oracle
perform Rust-side document matching
perform Rust-side numeric filtering
perform Rust-side phrase matching
call the Java oracle as implementation
call an external Vespa service
use fixture IDs/strings to generate results
parse YQL with scenario-specific regexes
bypass M3 for text leaves
```

A hand-written YQL parser for the supported subset is acceptable. A full Vespa YQL reimplementation is not required.

## Definition of done

M7 is complete when:

```text
M0–M6 remain green
frozen M7 API compiles
all supported typed queries canonicalize correctly
supported YQL expression and source-selection behavior matches pinned Java
YqlParser semantics within the M7 one-document-type boundary
all typed/YQL expression-equivalent pairs canonicalize equivalently
all typed and YQL paths return the same real-engine hit sets
M3 query analysis is proven in the real path
NOT/range/exists semantics match Vespa
negative field/type/YQL cases fail as specified
no Java runtime dependency exists in production
no Rust-side matcher/filter exists
verify-m7.sh passes
```
