# M2 contract — document mutation and integer-attribute search

## Goal

Reproduce through the frozen Rust API the already-proven direct C++ embedded
Vespa lifecycle using the `typea.attr1` INT32 attribute fixture.

This milestone deliberately does **not** add text analysis or a schema API.

## Required scenario

```text
PUT doc1 attr1=10
PUT doc2 attr1=20

SEARCH attr1=10
=> doc1

UPDATE doc2 attr1=10

SEARCH attr1=10
=> doc1, doc2

DELETE doc1
=> was_found = true

SEARCH attr1=10
=> doc2

SEARCH attr1=20
=> no hits
```

Document IDs are the real Vespa IDs:

```text
id:test:typea::1
id:test:typea::2
```

## Required path

```text
public searchkernel Rust API
    -> safe Rust native wrapper
    -> private native ABI
    -> real Vespa PersistenceEngine / DocumentDB matcher path
```

## Does not count

- storing document state in a Rust HashMap and answering search from Rust;
- implementing integer matching in Rust;
- invoking a fixture-specific C++ executable instead of the wrapped engine;
- hard-coding these document IDs or values in production code;
- returning the golden JSON without executing the mutations/searches;
- using a mock/fake backend in the acceptance path.

## Oracle

`oracle/embedded_crud_spike.cpp` is the preserved direct C++ scenario that was
already built and run successfully against the pinned Vespa revision.

Its assertions are the authority for the M2 golden output.
