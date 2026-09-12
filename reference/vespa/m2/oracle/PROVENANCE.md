# M2 oracle provenance

Pinned Vespa revision:

```text
2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

`embedded_crud_spike.cpp` is the previously built and executed direct C++
embedded Vespa spike.

It exercises real:

```text
proton::DocumentDB
proton::PersistenceHandlerProxy
proton::PersistenceEngine
feed put/update/remove
attribute search context
DocumentDB::match
in-process Vespa transaction log server
```

The assertions establish the M2 expected states:

```text
10 after put       -> doc1
10 after update    -> doc1, doc2
10 after delete    -> doc2
20 after update    -> none
```

The oracle does not establish any Rust API behavior. It establishes the native
behavior that the Rust wrapper must preserve.
