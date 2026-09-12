# M4 direct C++ oracle

The oracle is implemented by `m4_text_oracle.cpp`.

It reuses the lifecycle already proven by the M0 embedded spike:

```text
DocumentDB
PersistenceHandlerProxy
PersistenceEngine
in-process transaction log
DocumentDB::match
```

M4 adds only the text-specific pieces:

```text
M3 terms/spans
  -> StringFieldValue + linguistics SpanTree + TERM annotations
  -> real indexed STRING field

M3 query terms
  -> QueryBuilder StringTerm/Phrase
  -> StackDumpCreator
  -> real Matcher
```

`prepare-fixture.sh` clones the pinned DocumentDB unit-test config and freezes a
minimal M4 variant with:

- no M0 integer attribute;
- indexed `title` STRING field with positions;
- interleaved features for BM25 field statistics;
- `default` rank profile;
- `bm25` rank profile whose first phase is `bm25(title)`.

`build-cpp-oracle.sh` temporarily installs the source and a CMake target into
the pinned Vespa checkout, builds it using the already configured Vespa CMake
build tree, copies the executable under `oracle/target/`, and restores the
tracked upstream `CMakeLists.txt` before exiting.

The expected pinned source revision is:

```text
2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

## Authoring command

Set `VESPA_BUILD_DIR` if the build tree is not under `upstream/vespa`, then run:

```bash
./reference/vespa/m4/freeze-goldens.sh
```

This first validates frozen M3, prepares the M4 fixture, builds/runs the direct
C++ oracle, validates semantic invariants, writes `expected/results.jsonl`, and
finally writes `M4-IMMUTABLE.sha256`.

Review `expected/results.jsonl` before handing M4 to an implementation agent.
