# M0 source trace

The native SDK is derived from the already-proven embedded fixture built against:

```text
vespa-engine/vespa@2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

The proven fixture constructs and owns, in one process:

```text
proton::DocumentDB
proton::PersistenceHandlerProxy
proton::PersistenceEngine
search::transactionlog::TransLogServer
```

Relevant lifecycle calls in the preserved M2 oracle include:

```text
DocumentDB::create(...)
DocumentDB::start()
DocumentDB::waitForOnlineState()
PersistenceEngine::initialize()
DocumentDB::close()
```

M0's C++ adapter should encapsulate this setup rather than expose these classes
through the native header.
