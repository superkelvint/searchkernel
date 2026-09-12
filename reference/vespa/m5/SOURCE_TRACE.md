# M5 source trace

Pinned source:

```text
vespa-engine/vespa@2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

Search Kernel adapter:

```text
native/vespa/searchkernel_vespa.cpp
  EmbeddedEngine constructor: data/config paths, DocumentDB startup, bucket activation
  EmbeddedEngine destructor: handler removal, DocumentDB close, transport shutdown
  EmbeddedEngine::put/update/remove: PersistenceEngine operations and timestamps
  EmbeddedEngine::search: DocumentDB::match and document-ID conversion
```

Pinned Vespa lifecycle and persistence sources:

```text
searchcore/src/vespa/searchcore/proton/server/documentdb.cpp
  start(), startTransactionLogReplay(), close(), sync()
searchcore/src/vespa/searchcore/proton/server/feedhandler.cpp
  init(), appendOperation(), close(), syncTls(), replayTransactionLog()
searchlib/src/vespa/searchlib/transactionlog/translogserver.cpp
  TransLogServer construction and domain reopen
searchlib/src/vespa/searchlib/transactionlog/domain.cpp
  Domain construction, commit, sync, and destruction
searchlib/src/vespa/searchlib/transactionlog/domainpart.cpp
  persisted transaction-log part loading, commit, and fsync
searchcore/src/vespa/searchcore/proton/documentmetastore/documentmetastore.cpp
  persisted document metadata and attribute/index reopen state
```

Startup waits for Vespa online state and replays the transaction log before
the adapter accepts operations. Graceful `DocumentDB::close()` drains feed and
index work, and `FeedHandler::close()` synchronizes the transaction log.
