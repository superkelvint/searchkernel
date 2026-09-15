# M5 contract — persistence and reopen

## Goal

Prove that the embedded Vespa engine preserves the M2 `typea.attr1` logical
state across graceful engine and process lifetimes. M5 uses only the direct
`searchkernel-vespa-engine` safe wrapper and the existing native M2 mutation/search
path.

## Graceful process-boundary scenario

The verifier runs one driver binary in separate processes against one absolute
temporary directory:

```text
seed        put 1=10, 2=20, 3=30; update 2=10; remove 3; close
verify      reopen; expect 10=[1,2], 20=[], 30=[]; close
mutate      reopen; update 1=40; put 4=10; close
verify-final reopen; expect 10=[2,4], 40=[1]; close
```

The driver sorts returned IDs before emitting canonical JSON. The expected
results come from real native searches after each reopen.

## Durability semantics

`Engine` has no public close or checkpoint operation. Dropping it invokes the
native engine destructor, which closes `DocumentDB`; Vespa then synchronizes
the transaction log before the native handle is destroyed. Therefore a
successful mutation followed by graceful drop/reopen must expose the mutation
through the real M2 search path.

The embedded adapter uses Vespa's default transaction-log configuration, whose
per-commit fsync setting is disabled. A mutation return is consequently not a
promise of survival through power loss or an ungraceful process termination.
The bounded crash probe accepts the documented single-update prefix/post-state
outcomes and does not strengthen this guarantee.

## Errors and identity

Opening a new directory is valid. Reopening the same directory refers to the
same durable logical state, not to the same process object, pointer, thread
layout, cache, or runtime statistics. Invalid configuration, unusable paths,
and a deterministic corrupt persisted-state fixture must fail with native
context; they must not silently become a fresh index. In this pinned build the
corrupted transaction-log probe is a native non-zero failure rather than a
recoverable Rust error.

## Fixture

`fixtures/persisted-v1` is a Linux x86-64 compatibility fixture created with
the pinned Vespa revision and native ABI version 2. It contains puts for four
documents, an update, a delete, and the final logical state:

```text
doc2.attr1 = 10
doc4.attr1 = 10
doc1.attr1 = 40
doc3 = deleted
```

Expected searches are `10 -> [doc2, doc4]`, `20 -> []`, `30 -> []`, and
`40 -> [doc1]`.
