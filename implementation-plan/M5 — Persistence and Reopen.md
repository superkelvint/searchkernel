# M5 — Persistence and Reopen

M0–M2 are complete.

M3 and M4 may be in progress concurrently.

M5 depends on:

```text
M0
M1
M2
```

It does **not** depend on:

```text
M3
M4
```

Do not introduce a dependency on text indexing, linguistics, BM25, or later query functionality merely to satisfy M5.

---

# Goal

Verify that the embedded Vespa engine exposed through `searchkernel-vespa-engine` behaves as a persistent local index across process and engine lifetimes.

M5 establishes:

```text
mutate
search
close

reopen same directory
search
mutate
search
close
```

using the already-proven M2 integer-attribute path.

M5 does not invent durability semantics.

It observes, documents, and verifies the semantics provided by the embedded Vespa engine.

---

# 1. Dependency boundary

The M5 acceptance path is:

```text
Rust
    ↓
searchkernel-vespa-engine
    ↓
searchkernel-vespa-bindings
    ↓
native Search Kernel ABI
    ↓
real Vespa persistence/index state
```

Forbidden:

```text
searchkernel-ir
searchkernel
searchkernel-testkit

M3 linguistics
M4 text fixture

fake persistence
Rust-side database
```

M5 remains entirely within the M-series.

---

# 2. Persistence directory

Opening an engine against:

```text
/path/to/index
```

must associate all persistent engine state with that index directory or its explicitly documented native subdirectories.

The same directory reopened later must refer to the same persisted index.

Tests must use unique temporary directories.

Do not use shared global state.

---

# 3. Primary graceful-reopen scenario

Use the proven M2 integer attribute schema and mutation/search path.

Canonical scenario:

```text
PROCESS / SESSION A

open index directory

put:
    doc1 attr1 = 10
    doc2 attr1 = 20
    doc3 attr1 = 30

verify:
    search 10 -> doc1
    search 20 -> doc2
    search 30 -> doc3

update:
    doc2 attr1 = 10

delete:
    doc3

verify:
    search 10 -> doc1, doc2
    search 20 -> none
    search 30 -> none

close
```

Then:

```text
PROCESS / SESSION B

open same directory

verify:
    search 10 -> doc1, doc2
    search 20 -> none
    search 30 -> none

update:
    doc1 attr1 = 40

put:
    doc4 attr1 = 10

verify:
    search 10 -> doc2, doc4
    search 40 -> doc1

close
```

Then:

```text
PROCESS / SESSION C

open same directory

verify:
    search 10 -> doc2, doc4
    search 40 -> doc1

close
```

All queries must execute through the real M2 native search path.

---

# 4. Reopen semantics

Verify at least:

```text
open new directory
close
reopen

open
mutate
close
reopen

open
update existing document
close
reopen

open
delete document
close
reopen

multiple sequential reopen cycles
```

Persistent mutation state must survive graceful close/reopen according to actual Vespa behavior.

---

# 5. Process boundary

At least one acceptance scenario must cross an actual process boundary.

Do not prove persistence solely with:

```text
drop Engine
Engine::open(...)
```

inside one process.

Provide a small driver capable of modes such as:

```text
seed
verify
mutate
verify-final
```

Example:

```bash
m5-driver seed "$DIR"
m5-driver verify "$DIR"
m5-driver mutate "$DIR"
m5-driver verify-final "$DIR"
```

Each invocation must be a separate process.

This proves persisted state rather than accidental in-process lifetime state.

---

# 6. Graceful close first

The normative M5 requirement is graceful persistence:

```text
open
mutate
close cleanly
reopen
```

This must be implemented and frozen before crash/recovery scenarios are added.

Do not block basic M5 completion on exhaustive crash recovery.

---

# 7. Crash/recovery

After graceful reopen passes, add a bounded crash/recovery scenario where practical.

Use the same M2 attribute path.

Conceptually:

```text
PROCESS A

open
put/update/delete
ensure operations have reached the durability point
terminate without graceful engine close
```

then:

```text
PROCESS B

open same directory
observe recovered state
run searches
perform another mutation
close gracefully
```

The verifier must compare against **observed/documented Vespa durability semantics**.

Do not assert stronger guarantees than Vespa provides.

In particular, do not invent assumptions about:

```text
fsync timing
transaction durability
write acknowledgement guarantees
atomic multi-document transactions
```

unless they are established by the native implementation and oracle.

---

# 8. Durability point

Determine from the existing embedded Vespa/native implementation what successful mutation completion means with respect to persistence.

Document this briefly in:

```text
reference/vespa/m5/CONTRACT.md
```

and:

```text
reference/vespa/m5/SOURCE_TRACE.md
```

M5 should answer:

> After a successful mutation call and the relevant supported shutdown/crash condition, what state is guaranteed to be observable after reopen?

Do not redesign that contract.

---

# 9. Persistent compatibility fixture

Create at least one preserved M5 index fixture for future Vespa upgrade testing.

The initial fixture should deliberately remain simple and depend only on M2.

Record:

```text
Search Kernel revision
native ABI version
pinned Vespa revision
target/platform where relevant
schema/fixture definition
mutation history
expected final logical contents
expected search results
```

The fixture should include:

```text
inserts
update
delete
multiple surviving documents
multiple attribute values
```

Do not require M4 text state in the initial fixture.

---

# 10. Fixture portability

A persistent fixture is an upgrade/compatibility artifact, not ordinary test scratch state.

Store enough metadata to determine:

```text
who created it
with which Vespa revision
with which Search Kernel version
what logical state it contains
how to verify it
```

Do not assume persisted engine files are portable between:

```text
architectures
operating systems
Vespa revisions
```

unless verified.

Platform-specific fixtures are acceptable.

---

# 11. Future M4 convergence

M4 is not a prerequisite for M5.

Once M4 is complete, add an additional cumulative scenario covering text persistence:

```text
open

index text documents
term/phrase search
update text
delete text

close

reopen

repeat searches
verify old postings remain absent
verify new postings survive
```

This is:

```text
M4 + M5 integration coverage
```

not a reason to delay M5.

The initial M5 acceptance must remain runnable using only the M2 path.

---

# 12. Future schema/config convergence

Likewise, M6 is not required for M5.

M5 may use the same proven fixed/native configuration used by M2.

Once M6 lands, add separate reopen tests for dynamically generated configuration as appropriate.

Do not expand M5 scope to solve M6.

---

# 13. Engine identity

Verify reopening means:

```text
same logical index directory
    ->
same persisted document/index state
```

but not:

```text
same process object identity
same native pointer
same thread layout
same transient runtime state
```

Only durable logical state is part of M5.

---

# 14. State that must persist

For the supported M2 fixture, verify persistence of the logical effects of:

```text
put
update
delete
```

and whatever native state is necessary for subsequent real searches to return the same logical results.

Do not inspect or freeze incidental internal Vespa file layouts unless necessary for upgrade diagnostics.

---

# 15. State that is not part of M5

M5 does not specify persistence of:

```text
query caches
temporary memory
thread state
runtime handles
statistics counters
transient search state
```

It also does not define:

```text
schema migrations
online schema changes
multi-index transactions
replication
snapshots
backup/restore APIs
distributed recovery
```

---

# 16. Error cases

Add tests for at least:

```text
open nonexistent/new directory
reopen valid existing directory

invalid/unusable directory
permission failure where practical

corrupted or incompatible state
    -> explicit error, where a deterministic fixture is practical
```

Do not silently initialize a fresh index over state that the native engine recognizes as invalid.

Preserve useful native error context through the Rust error boundary.

---

# 17. Repeated reopen

Run a deterministic loop such as:

```text
for N cycles:

    open
    mutate one document
    verify
    close
```

Then:

```text
open
verify complete final state
close
```

This should catch lifecycle/reopen bugs that a single cycle misses.

Keep the test bounded and deterministic.

---

# 18. Resource cleanup

Graceful close/drop must release resources sufficiently for the same directory to be immediately reopened by a new engine/process.

Verify no stale ownership prevents normal reopen.

Where practical, check for obvious leakage of:

```text
file descriptors
threads
locks
native handles
```

Long-duration leak testing belongs primarily to M11.

M5 only needs enough coverage to prove normal repeated reopen.

---

# 19. No persistence reimplementation

Forbidden production shortcuts include:

```text
Rust sidecar database
JSON snapshot files
custom WAL
manual document replay
test-only persisted result cache
fixture-specific restore logic
```

Persisted state must come from the embedded Vespa engine.

---

# 20. No fake verification

The verifier must prove that search after reopen goes through the real native M2 path.

Do not verify persistence by reading:

```text
test metadata
fixture JSON
custom Rust files
```

The logical assertions must be obtained by executing real operations after reopening the engine.

---

# 21. Oracle / authority

M5 is primarily verified against **real embedded Vespa behavior**, not a separately reimplemented oracle.

Use:

```text
known logical mutations
        ↓
real engine
        ↓
close/reopen
        ↓
real engine query
        ↓
expected logical state
```

Where crash semantics are ambiguous, inspect the pinned Vespa implementation and document the actual supported guarantee.

The oracle must not be a Rust persistence implementation.

---

# 22. Required harness

Create:

```text
reference/vespa/m5/
    CONTRACT.md
    SOURCE_TRACE.md

    scenarios/
        graceful-reopen/
        repeated-reopen/
        crash-recovery/        # if included in initial M5

    expected/

    fixtures/
        persisted-v1/          # preserved upgrade fixture

    verify-m5.sh
```

The exact layout may vary slightly if existing M-series harness conventions require it.

Keep it consistent with earlier M milestones.

---

# 23. `SOURCE_TRACE.md`

Keep this short.

Record the pinned upstream/native source locations responsible for:

```text
engine open
engine close
transaction log / durability
document persistence
recovery/replay
index/attribute reopen
```

M5 does not require porting this code.

The trace exists so future reviewers can identify where the observed durability behavior originates.

---

# 24. Canonical output

Each scenario should emit deterministic logical JSON rather than native filesystem details.

Example:

```json
{
  "search_10": ["doc2", "doc4"],
  "search_20": [],
  "search_30": [],
  "search_40": ["doc1"]
}
```

Canonicalize result ordering where the M2 contract already requires it.

Do not include unstable details such as:

```text
native pointers
thread IDs
timestamps
temporary paths
internal generation numbers
```

unless specifically required by the contract.

---

# 25. Verification sequencing

M5 verification must first prove its prerequisites:

```text
M0
M1
M2
```

then execute M5.

It does **not** run:

```text
M3
M4
```

as prerequisites.

Conceptually:

```bash
verify-m0.sh
verify-m1.sh
verify-m2.sh
verify-m5-specific.sh
```

or the equivalent cumulative harness.

This is intentional.

M3/M4 may be broken, incomplete, or running on another branch without preventing M5 development.

---

# 26. Concurrent development rule

While M3/M4 are being implemented:

```text
M5 may modify:
    lifecycle/persistence harness
    low-level engine reopen plumbing
    persistence tests
    M5-specific native adapter code where needed
```

M5 must not modify or depend on:

```text
M3 linguistics contracts
M4 text contracts
M3/M4 fixtures
M3/M4 expected output
```

Likewise, M3/M4 should not rewrite M5 persistence semantics.

If concurrent branches touch common lifecycle code, preserve the previously frozen M1/M2 API and resolve integration mechanically.

---

# 27. Public API scope

Add the minimum low-level API required to express persistent reopen behavior.

Prefer using the existing:

```rust
Engine::open(...)
drop(engine)
```

contract.

Do not introduce:

```text
PersistenceManager
TransactionLog
Checkpoint API
RecoveryManager
```

unless the native engine genuinely requires a public Rust operation that cannot be represented through existing lifecycle calls.

Internal implementation helpers are fine.

Public abstractions require explicit justification.

---

# 28. Acceptance commands

Run the normal M-series acceptance plus:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
./reference/vespa/m5/verify-m5.sh
```

If the repository's M-series verifier convention uses a different top-level path, follow that convention.

---

# 29. Structural verifier requirements

The M5 verifier should prove:

```text
real native engine is opened

real M2 mutations are used

real M2 search is used after reopen

the same filesystem directory crosses process boundaries

no Rust persistence implementation participates

no M3/M4 dependency is required

persistent fixture metadata identifies Vespa revision

graceful reopen works across processes

multiple reopen cycles work
```

For crash recovery, also prove the writer process terminates without executing the ordinary graceful close path.

---

# 30. Completion criteria

M5 is complete when:

1. a real embedded index can be mutated and gracefully closed;
2. a new process can reopen the same directory;
3. inserts survive reopen;
4. updates survive reopen;
5. deletes survive reopen;
6. real M2 searches observe the expected state after reopen;
7. further mutations after reopen work;
8. another reopen observes those later mutations;
9. repeated reopen cycles work;
10. at least one preserved persistent compatibility fixture exists;
11. actual durability/recovery semantics are documented;
12. crash/recovery is tested where practical without inventing guarantees;
13. no M3/M4 implementation is required;
14. no Rust persistence subsystem has been introduced;
15. fmt, clippy, tests, and the M5 verifier pass.

---

# 31. Fundamental rule

M5 answers:

> Can the already-working embedded engine state survive lifecycle and process boundaries correctly?

It does **not** answer:

> Does text indexing work?

That belongs to M4.

The parallel development shape is:

```text
             ┌──► M3 ──► M4
             │
M2 ──────────┤
             │
             └──► M5
```

Once both paths exist, add cumulative coverage:

```text
M4 + M5
    ↓
text indexing across reopen
```

without retroactively making M4 a prerequisite of M5.