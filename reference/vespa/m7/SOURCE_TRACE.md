# M7 source trace

Pinned Vespa source:

```text
vespa-engine/vespa@2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

## Java YQL oracle

```text
container-search/src/main/java/com/yahoo/search/yql/YqlParser.java
container-search/src/main/java/com/yahoo/search/query/parser/{ParserEnvironment,Parsable}.java
container-search/src/main/java/com/yahoo/search/query/QueryTree.java

container-search/src/main/java/com/yahoo/prelude/{Index,IndexFacts,IndexModel,SearchDefinition}.java
container-search/src/main/java/com/yahoo/prelude/query/
    AndItem.java
    OrItem.java
    NotItem.java
    PhraseItem.java
    WordItem.java
    IntItem.java
    Limit.java
    TrueItem.java

opennlp-linguistics/src/main/java/com/yahoo/language/opennlp/OpenNlpLinguistics.java
```

The Java oracle constructs `IndexFacts` programmatically for the small M7 field subset and invokes the real pinned `YqlParser` with `OpenNlpLinguistics`.

It projects the resulting query tree into the M7 canonical AST. It does not implement YQL semantics itself.

## Native execution path

M7 extends the real path already proven by M4:

```text
searchlib/src/vespa/searchlib/query/tree/querybuilder.h
searchlib/src/vespa/searchlib/query/tree/stackdumpcreator.*
searchcore/src/vespa/searchcore/proton/matching/
searchcore/src/vespa/searchcore/proton/server/documentdb.*
```

M7 adds boolean and numeric query nodes to the M4 term/phrase path. Actual matching remains Vespa C++.

## M3 handoff

M3 remains the authority for query-time tokenization, normalization and stemming. The Java YQL oracle is used for YQL structural semantics; production Rust text leaves are analyzed by M3 before native lowering.

## M6 handoff

M6's private compiled-schema manifest is the field metadata authority used for M7 field/type validation. M7 does not introduce another schema registry.
