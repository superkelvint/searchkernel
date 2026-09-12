# M4 source trace

Pinned Vespa source:

```text
vespa-engine/vespa@2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

The direct oracle is an extension of the already-proven M0 embedded fixture and
uses the pinned implementations of:

```text
searchcore/src/vespa/searchcore/proton/server/documentdb.*
    proton::DocumentDB

searchcore/src/vespa/searchcore/proton/persistenceengine/
    proton::PersistenceEngine
    proton::PersistenceHandlerProxy

searchcore/src/vespa/searchcore/proton/matching/
    ProtonNodeTypes / Matcher path

searchlib/src/vespa/searchlib/query/tree/querybuilder.h
searchlib/src/vespa/searchlib/query/tree/stackdumpcreator.*
    QueryBuilder
    StringTerm / Phrase
    StackDumpCreator

document/src/vespa/document/fieldvalue/stringfieldvalue.*
document/src/vespa/document/annotation/{span,spanlist,spantree,annotation}.*
document/src/vespa/document/datatype/annotationtype.*
    StringFieldValue
    Span / SpanList / SpanTree
    TERM annotations

searchcore/src/tests/proton/documentdb/cfg/
    base precompiled DocumentDB test configuration
```

The M0 embedded spike proves the same in-process lifecycle used here:

```text
DocumentDB
PersistenceEngine
put/update/remove
DocumentDB::match
QueryBuilder + StackDumpCreator
```

M4 does not introduce another engine or mock path.

## M3 handoff

The authoritative M3 Java oracle supplies:

```text
term
byte_start
byte_len
logical position
```

for document and query text while M4 goldens are authored. The C++ oracle does
not tokenize, normalize, lowercase, accent-fold, or stem.

Document terms are lowered to the standard `linguistics` span tree. If an
analyzed term differs from its source substring, its TERM annotation contains a
StringFieldValue with the analyzed term; otherwise the TERM annotation has no
value. Query terms are lowered directly to real `StringTerm`/`Phrase` nodes.

## Frozen text fixture

`oracle/prepare-fixture.sh` clones the pinned upstream DocumentDB test fixture
and freezes the M4 variant under:

```text
reference/vespa/m4/fixture/embedded_text_cfg/
```

The generated fixture defines indexed `title`, positions, interleaved features,
and a `bm25` rank profile using Vespa's `bm25(title)` rank feature. Config
compilation remains explicitly out of scope until M6.
