# Authoring the M8 oracle

Implement `m8_local_oracle.cpp` according to `ORACLE_SPEC.md` by adapting the frozen M4 direct C++ lifecycle, not Search Kernel production code.

Recommended workflow:

1. check out/verify the exact `PINNED_VESPA.json` revision;
2. inspect the pinned `SearchRequest`, `SearchReply`, `DocsumRequest`, `DocsumReply`, and `DocumentDB` APIs recorded in `SOURCE_TRACE.md`;
3. adapt M4's `DocumentDB` lifecycle and M3-analysis bridge;
4. implement all eight positive scenarios generically from scenario JSON/protocol input;
5. build with `build-cpp-oracle.sh` and record provenance;
6. run `freeze-goldens.sh`;
7. manually inspect anti-shortcut scenarios;
8. run `freeze-immutable.sh` only after review.

The implementation agent for M8 must not author or modify this oracle.
