# Vespa Release Upgrade — End-to-End Qualification

## 1. Purpose

A new Vespa release must not be adopted merely because Search Kernel compiles against it.

An upgrade is acceptable only when the candidate Vespa revision is proven compatible with the specific Vespa behavior Search Kernel depends on.

The qualification target is:

```text
current released Search Kernel
        +
candidate Vespa revision
        ↓
same supported Search Kernel behavior
```

The suite verifies the complete embedded path:

```text
Search Kernel Rust API
        ↓
searchkernel-vespa-engine
        ↓
native ABI
        ↓
Search Kernel C++ adapter
        ↓
candidate Vespa C++
        ↓
persistent state / indexing / matching / ranking
```

The suite is a **release gate**.

A candidate Vespa revision that fails any mandatory compatibility contract is not automatically adopted.

Remaining on the currently pinned Vespa revision is always valid.

---

# 2. What compatibility means

Qualification covers three distinct questions.

## 2.1 Integration compatibility

Does Search Kernel still correctly drive the candidate Vespa engine?

```text
Search Kernel
    ↓
candidate Vespa
```

This includes:

```text
native lifecycle
configuration
documents
updates
linguistic annotations
query trees
matching
attributes
docsums
ranking
persistence
recovery
```

Compilation alone does not satisfy this requirement.

---

## 2.2 Search Kernel behavioral compatibility

Does upgrading Vespa unexpectedly change behavior Search Kernel promises?

Compare:

```text
released Search Kernel + current Vespa
```

with:

```text
same Search Kernel + candidate Vespa
```

Product-visible behavior must remain compatible unless a behavior change is explicitly reviewed and accepted.

---

## 2.3 Persistent-state compatibility

Can the candidate release safely operate on indexes created by the currently released Search Kernel/Vespa combination?

The mandatory transition is:

```text
current Vespa
    ↓
persistent index
    ↓
candidate Vespa
```

The upgrade suite must not rely only on newly created indexes.

---

# 3. Authority model

The upgrade suite uses three authorities.

```text
A. candidate upstream Vespa behavior

B. Search Kernel behavior on the currently pinned Vespa revision

C. frozen Search Kernel compatibility contracts
```

They answer different questions.

### Upstream parity

Where Search Kernel replaces Vespa Java-side functionality:

```text
Vespa Java implementation
        vs
Search Kernel replacement
```

Examples:

```text
linguistics
configuration generation
query preparation where applicable
```

### Native-path parity

Where Vespa C++ owns execution:

```text
expected native behavior
        vs
Search Kernel → candidate Vespa
```

### Product stability

For public Search Kernel behavior:

```text
Search Kernel + current Vespa
        vs
Search Kernel + candidate Vespa
```

No single oracle replaces all three.

---

# 4. Compatibility assertion classes

Every assertion must be classified explicitly.

## EXACT

Canonical output must be identical.

Examples:

```text
document IDs
field values
presence/absence
mutation results
error category
facet counts
stored-field contents
```

Failure blocks release.

---

## SEMANTIC

Representation may change but observable meaning must remain equivalent.

Examples:

```text
generated internal configuration
internal query representation
native diagnostic text
non-contractual metadata
```

Tests must compare canonical meaning, not raw bytes.

Failure blocks release.

---

## ORDERED

Result membership and ordering must remain identical, but floating-point scores need not be bit-identical.

Examples:

```text
BM25 result order
field-weight effects
ranking tie behavior promised by Search Kernel
```

Failure blocks release.

---

## BOUNDED

Values may change within a frozen tolerance.

Examples:

```text
floating-point ranking values
startup time
RSS/PSS
index size
thread count
file descriptors
```

Exceeding a hard bound blocks release.

A smaller warning threshold may require review without failing qualification.

---

## OBSERVATIONAL

Behavior is recorded for comparison but is not currently contractual.

These observations may not participate in automatic release decisions.

An observational result must never silently become a release requirement.

---

# 5. Repository layout

Maintain the suite under:

```text
reference/vespa/upgrade/
```

Recommended structure:

```text
reference/vespa/upgrade/
├── CONTRACT.md
├── COMPATIBILITY_MATRIX.md
├── verify-upgrade.sh
├── candidate/
├── scenarios/
│   ├── lifecycle/
│   ├── feed/
│   ├── linguistics/
│   ├── text/
│   ├── query/
│   ├── schema/
│   ├── attributes/
│   ├── results/
│   ├── ranking/
│   ├── persistence/
│   ├── recovery/
│   ├── concurrency/
│   └── resources/
├── fixtures/
│   ├── persistent/
│   └── corpora/
├── expected/
└── tools/
```

The compatibility corpus is immutable during qualification.

Do not alter expected output merely to make a candidate Vespa release pass.

---

# 6. Candidate identity

Every qualification run records:

```text
Search Kernel revision
Search Kernel release version
current Vespa revision
candidate Vespa revision
candidate Vespa release/tag
native ABI version
target triple
compiler versions
build mode
qualification corpus version
```

The final report must uniquely identify the exact candidate that was tested.

---

# 7. Phase 0 — Build and structural compatibility

The candidate must first successfully produce the Search Kernel native SDK.

Verify:

```text
candidate Vespa builds
Search Kernel C++ adapter builds
native SDK packages successfully
Rust workspace links against candidate SDK
expected native symbols exist
runtime dependencies are acceptable
```

Also verify the native dependency boundary:

```text
no Vespa headers leak into public Rust API
no new raw FFI outside searchkernel-vespa-bindings
no Vespa C++ type crosses the Search Kernel ABI
```

Passing this phase proves only that qualification can continue.

It does **not** establish behavioral compatibility.

---

# 8. Phase 1 — Engine lifecycle

Exercise the actual lifecycle used by Search Kernel.

Required scenarios:

```text
open
close

open
close
open
close

create
write
search
close
reopen
search
close
```

Stress:

```text
100+ repeated open/close cycles
failed initialization followed by successful initialization
empty index
non-empty index
```

Verify:

```text
startup succeeds
shutdown completes
feed/index work drains as expected
no deadlock
no crash
no leaked process-global state preventing reopen
```

Hard failure blocks release.

---

# 9. Phase 2 — Feed and mutation semantics

Exercise every native mutation operation Search Kernel uses.

Minimum scenarios:

```text
PUT
UPSERT/replace semantics where exposed
field UPDATE
DELETE
GET where exposed
```

Required state transition:

```text
PUT doc1 value=A
SEARCH A -> doc1

UPDATE doc1 value=B
SEARCH A -> none
SEARCH B -> doc1

DELETE doc1
SEARCH B -> none
```

Additional cases:

```text
multiple documents
multiple updates to same document
delete nonexistent document
replace existing document
mutation of multiple field types
rapid sequential mutations to same document
```

Verify both operation acknowledgement and subsequently observable engine state.

A native call returning success is insufficient if the resulting search state is incorrect.

---

# 10. Phase 3 — Linguistics/index annotation compatibility

Search Kernel supplies linguistics that Vespa normally receives from functionality outside the retained C++ engine.

Qualification must therefore verify both:

```text
Search Kernel linguistic output
```

and:

```text
candidate Vespa interpretation of that output
```

Use frozen cases covering at least:

```text
case normalization
punctuation
multiple whitespace
Unicode
accented characters
stemming
empty strings
repeated terms
token positions
phrase boundaries
mixed punctuation
```

For each case:

```text
raw document
    ↓
Search Kernel analysis
    ↓
annotations supplied to Vespa
    ↓
candidate Vespa index
    ↓
queries
    ↓
expected matches
```

The Java oracle remains authoritative for the supported linguistic subset.

The upgrade qualification additionally proves that the candidate C++ indexing path still consumes those annotations compatibly.

---

# 11. Phase 4 — Text indexing and matching

Use real candidate Vespa indexing and matching.

Corpus must include documents that distinguish:

```text
single terms
multiple terms
term frequency
phrase positions
word order
repeated words
updates
deletes
```

Queries include:

```text
term
phrase
term after update
phrase after update
term after delete
```

Verify:

```text
hit membership
hit ordering where ranking applies
term removal after update
posting removal after delete
phrase semantics
BM25 execution
```

No Rust-side matching participates in qualification.

---

# 12. Phase 5 — Schema/configuration compatibility

For every low-level schema capability supported by Search Kernel, instantiate the candidate engine from Search Kernel-generated configuration.

Minimum schema matrix:

```text
text:index

text:index + stored

keyword/string:attribute

keyword/string:attribute + stored

integer:attribute

floating-point:attribute

mixed text + attribute fields

BM25-enabled text

multiple text fields

searchable + non-searchable combinations

stored + non-stored combinations
```

Each schema fixture must be tested behaviorally:

```text
compile Search Kernel schema
        ↓
candidate Proton configuration
        ↓
start candidate engine
        ↓
insert documents
        ↓
exercise declared capabilities
```

Raw generated configuration does not need to be byte-identical to previous Vespa releases.

The resulting semantics must remain compatible.

---

# 13. Phase 6 — Query compatibility

Exercise every query construct Search Kernel emits.

Minimum matrix:

```text
Term
Phrase

And
Or
Not

Equals
Range
Exists
```

Compositions include:

```text
nested AND/OR
NOT inside AND
text query + exact filter
text query + numeric range
multiple filters
phrase + filter
```

Boundary cases include:

```text
unknown field
wrong field type
empty term where representable
minimum numeric value
maximum numeric value
inclusive/exclusive range boundaries
missing attribute
```

Verification is execution-based:

```text
Search Kernel query
    ↓
candidate Vespa query representation
    ↓
candidate Matcher
    ↓
canonical hits
```

Successful query construction alone is insufficient.

---

# 14. Phase 7 — Attributes

Exercise every attribute type and operation Search Kernel actually exposes.

For each supported type verify:

```text
feed
lookup/filter
update
sort
ranking access where supported
reopen
```

Include:

```text
missing value
zero
negative number
large number
empty string where meaningful
Unicode string
```

The suite must detect changes in missing-value semantics, update behavior, filtering, or sorting.

---

# 15. Phase 8 — Results and docsums

Exercise the complete search-result path Search Kernel uses.

Verify:

```text
hit document identity
score presence
total hits
stored fields
field types
missing fields
projection
sorting
limit
offset where supported
```

Where docsums are involved:

```text
match
    ↓
hit identity
    ↓
summary/docsum retrieval
    ↓
Search Kernel result
```

must be tested end to end.

A correct match followed by incorrect document filling fails qualification.

---

# 16. Phase 9 — Ranking

Use small deterministic corpora specifically designed to expose ranking changes.

Example:

```text
doc1 title="red red bicycle"
doc2 title="red bicycle"
doc3 title="bicycle"
```

Test all ranking functionality Search Kernel currently uses:

```text
BM25
field-specific weighting
attribute features
query features
rank properties
ranking expressions
feature inspection
```

Assertions should normally be:

```text
ORDERED:
    result order

EXACT:
    feature presence/name where contractual

SEMANTIC:
    feature meaning

BOUNDED:
    floating-point values where exact equality is unnecessary
```

Do not fail an upgrade solely because an unpromised internal floating-point score changes.

Do fail if:

```text
hit ordering changes unexpectedly
required ranking feature disappears
query feature is ignored
field weighting changes semantics
rank property is no longer honored
```

---

# 17. Phase 10 — Persistent-index forward compatibility

Persistent fixtures are first-class release artifacts.

Maintain representative index directories created by the currently released Search Kernel using the currently pinned Vespa revision.

Each fixture records:

```text
Search Kernel version
Vespa revision
schema
documents
mutation history
expected searches
expected stored values
```

Qualification performs:

```text
copy released fixture
        ↓
open with candidate Vespa
        ↓
search
        ↓
update
        ↓
search
        ↓
delete
        ↓
search
        ↓
close
        ↓
reopen candidate
        ↓
search
```

The fixture copy must not be mutated in place.

Failure to open or correctly operate on a supported released index blocks automatic adoption.

---

# 18. Phase 11 — Candidate persistence/reopen

Create fresh state using the candidate.

Run:

```text
open
put
update
delete
search
close

open same directory
search
update
search
close

open again
verify
```

This is distinct from forward compatibility.

Both must pass.

---

# 19. Phase 12 — Crash/recovery

Crash behavior must match the durability semantics Search Kernel intentionally supports.

Use subprocess isolation.

Scenarios should include:

```text
write
graceful close
restart
```

and bounded abrupt termination probes such as:

```text
open
perform acknowledged mutations
SIGKILL
restart
inspect
```

Additional stress scenarios:

```text
kill during sustained feeding
kill after update
kill after delete
repeated kill/restart cycles
```

The expected behavior must not claim stronger durability than embedded Vespa actually provides.

Graceful persistence guarantees and abrupt-termination observations must be documented separately.

A deterministic regression against the supported recovery contract blocks release.

---

# 20. Phase 13 — Concurrency

Exercise only concurrency modes Search Kernel intends to support.

Minimum workload:

```text
1 writer
N concurrent searchers
```

Add where supported:

```text
concurrent updates to separate documents
search during updates
rapid repeated updates to one document
close only after workers have stopped
```

Run for a sufficiently large operation count to expose lifecycle and synchronization failures.

Verify:

```text
no crash
no deadlock
no use-after-free
no corrupted results
no state violation beyond documented visibility semantics
```

Run sanitizer-enabled native builds where practical.

---

# 21. Phase 14 — Long-running state-machine workload

Run a deterministic seeded mutation/search workload.

Model only externally observable state necessary to validate correctness.

Operations:

```text
put
update
delete
search exact attribute
search text
filter
sort
close/reopen periodically
```

The reference state machine may determine expected document membership and field state.

It must not implement Vespa ranking or search algorithms.

Record the random seed.

Any failing seed becomes a permanent regression fixture.

---

# 22. Phase 15 — Resource compatibility

Embedded operation makes resource behavior part of practical compatibility.

Measure:

```text
startup latency
shutdown latency
RSS
PSS where available
thread count
open file descriptors
persistent index size
disk growth during mutation
```

Use representative:

```text
empty index
small index
medium qualification corpus
long-running mutation workload
```

Each measurement has:

```text
hard maximum
warning regression percentage
```

Example policy:

```text
hard:
    no >2x RSS regression

warning:
    >20% RSS regression

hard:
    no >2x startup regression

warning:
    >25% startup regression
```

Actual thresholds must be frozen from measured Search Kernel requirements.

Warnings prevent unattended promotion until reviewed.

Hard failures reject the candidate.

---

# 23. Phase 16 — Existing M-series regression

The candidate must pass the complete low-level milestone suite:

```text
M0
M1
M2
...
latest completed M milestone
```

Earlier frozen verifiers remain unchanged.

This ensures the candidate remains compatible with every previously accepted low-level capability.

---

# 24. Phase 17 — Existing I-series regression

Run every completed integration verifier against the candidate Vespa SDK.

The acceptance path must use:

```text
real Search Kernel high-level API
        ↓
real searchkernel-ir-adapter
        ↓
real searchkernel-vespa-engine
        ↓
candidate Vespa
```

No:

```text
RecordingBackend
ScriptedBackend
mock search engine
```

may participate in upgrade qualification.

---

# 25. Differential run against current Vespa

Run the canonical upgrade corpus twice:

```text
RUN A
Search Kernel revision X
current pinned Vespa

RUN B
Search Kernel revision X
candidate Vespa
```

Canonicalize both outputs.

Then compare according to each assertion's classification:

```text
EXACT
SEMANTIC
ORDERED
BOUNDED
OBSERVATIONAL
```

Every difference must be either:

```text
expected and already encoded by the contract
```

or:

```text
a qualification failure
```

There is no automatic category for:

```text
"probably harmless"
```

Unknown differences block automatic release.

---

# 26. No candidate-specific goldens

Do not create different expected Search Kernel behavior merely because the candidate Vespa version behaves differently.

Forbidden:

```text
if Vespa >= X:
    expect different Search Kernel result
```

unless Search Kernel has deliberately adopted a version-dependent behavior and that decision is separately specified.

Candidate-specific implementation adaptations inside the private C++ adapter are acceptable.

Candidate-specific public semantics are not.

---

# 27. Native adapter changes

A Vespa release may require Search Kernel's C++ adapter to change.

That is expected.

The adapter may adapt:

```text
constructor changes
type changes
namespace changes
configuration plumbing
native call sequences
internal result extraction
```

provided all compatibility contracts continue to pass.

The native ABI exposed to Rust should remain stable unless Search Kernel itself intentionally changes it.

---

# 28. Upgrade qualification command

Provide a single entry point:

```bash
./reference/vespa/upgrade/verify-upgrade.sh \
    --current <revision> \
    --candidate <revision>
```

The verifier owns orchestration.

It must not regenerate expected results.

The verifier must exit non-zero for:

```text
build failure
hard compatibility failure
unknown differential result
persistent fixture failure
sanitizer failure
hard resource regression
incomplete qualification
```

---

# 29. Qualification report

Every run produces a machine-readable report:

```text
upgrade-report.json
```

and a human-readable summary:

```text
upgrade-report.md
```

Minimum report fields:

```text
candidate identity

build status

contracts:
    passed
    failed
    warnings

M-series status
I-series status

persistent fixture status
recovery status
sanitizer status

resource comparison

differential changes

final qualification state
```

Final state is exactly one of:

```text
QUALIFIED
REJECTED
REVIEW_REQUIRED
INCOMPLETE
```

---

# 30. Automatic release policy

A candidate may be automatically promoted only when:

```text
qualification == QUALIFIED
```

which requires:

```text
native SDK build passes

all mandatory lifecycle contracts pass

all feed contracts pass

all linguistics/index contracts pass

all query contracts pass

all schema contracts pass

all result/docsum contracts pass

all ranking contracts pass

all current persistent fixtures pass

candidate reopen tests pass

supported recovery contracts pass

concurrency tests pass

state-machine workload passes

sanitizers pass

all completed M-series verifiers pass

all completed I-series verifiers pass

no hard resource regression

no unresolved warning requiring review

no unknown differential result
```

Then:

```text
candidate Vespa revision
        ↓
update pinned revision
        ↓
build release artifacts
        ↓
rerun qualification from pinned release commit
        ↓
publish Search Kernel release
```

The final published artifact must be built from the exact Vespa revision that was qualified.

---

# 31. Failure policy

A failed candidate does not require Search Kernel to upgrade.

Possible outcomes are:

```text
fix private Search Kernel adapter
and rerun

identify Search Kernel bug
fix and rerun

accept an intentional Search Kernel semantic change
update contract explicitly
and rerun

report/upstream Vespa regression
remain pinned
```

Do not weaken a compatibility contract solely to consume a newer Vespa release.

---

# 32. Compatibility matrix

Maintain:

```text
reference/vespa/upgrade/COMPATIBILITY_MATRIX.md
```

Every Vespa capability Search Kernel relies upon must map to at least one executable qualification contract.

Example:

| Search Kernel dependency | Qualification coverage |
|---|---|
| Engine lifecycle | lifecycle |
| Feed API | feed |
| Transaction/reopen behavior | persistence, recovery |
| Annotated text indexing | linguistics, text |
| Matcher | text, query |
| Query tree representation | query |
| Proton configuration | schema |
| Attributes | attributes |
| Docsums | results |
| BM25 | ranking |
| Ranking features | ranking |
| Persistent format | persistence |
| Concurrency assumptions | concurrency |

No native capability should enter production use without corresponding upgrade coverage.

---

# 33. Definition of done for Vespa adoption

A Vespa upgrade is complete only when:

```text
candidate revision selected

native SDK built

adapter changes complete

all qualification phases run

all hard contracts pass

all differential output classified

released persistent fixtures pass

resource limits pass

qualification report == QUALIFIED

pinned Vespa revision updated

full suite rerun from final release commit

release artifacts built from qualified revision
```

Compilation is a prerequisite.

It is never evidence of compatibility by itself.

---

# 34. Guiding invariant

The upgrade system exists to establish one claim:

> For every Vespa capability that Search Kernel relies upon, the candidate Vespa release has been exercised through the same embedded path Search Kernel uses in production and has satisfied the frozen behavioral contract for that capability.

If the suite cannot establish that claim, Search Kernel remains on the existing Vespa revision.