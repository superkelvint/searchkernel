# M2 source trace

Pinned source:

```text
vespa-engine/vespa@2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

The direct oracle preserved in `oracle/embedded_crud_spike.cpp` exercises:

```text
PersistenceEngine::put(...)
PersistenceEngine::update(...)
PersistenceEngine::remove(...)
DocumentDB::match(...)
```

The direct query is built with Vespa's query tree builder as a number term on
`attr1`, serialized into `SearchRequest`, with the `default` rank profile.

The Rust wrapper should map its M2 public types onto this behavior. It should
not create a second Rust-side document/search implementation.
