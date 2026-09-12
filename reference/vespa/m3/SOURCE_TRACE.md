# M3 source trace

The checked-in/pinned `upstream/vespa` checkout is normative. Record its Git revision when the harness is frozen:

```bash
git -C upstream/vespa rev-parse HEAD
```

The M3 oracle intentionally compiles against that checkout rather than a Maven Central Vespa release.

Inspect and retain compatibility with the pinned versions of these files/classes:

```text
opennlp-linguistics/src/main/java/com/yahoo/language/opennlp/OpenNlpLinguistics.java
    com.yahoo.language.opennlp.OpenNlpLinguistics

opennlp-linguistics/src/main/java/com/yahoo/language/opennlp/OpenNlpTokenizer.java
    com.yahoo.language.opennlp.OpenNlpTokenizer

linguistics/src/main/java/com/yahoo/language/Linguistics.java
    com.yahoo.language.Linguistics

linguistics/src/main/java/com/yahoo/language/process/Tokenizer.java
    com.yahoo.language.process.Tokenizer

linguistics/src/main/java/com/yahoo/language/process/Token.java
    com.yahoo.language.process.Token

linguistics/src/main/java/com/yahoo/language/process/Stemmer.java
    com.yahoo.language.process.Stemmer

linguistics/src/main/java/com/yahoo/language/process/StemmerImpl.java
    com.yahoo.language.process.StemmerImpl

linguistics/src/main/java/com/yahoo/language/process/StemList.java
    com.yahoo.language.process.StemList

linguistics/src/main/java/com/yahoo/language/process/StemMode.java
    com.yahoo.language.process.StemMode

linguistics/src/main/java/com/yahoo/language/simple/SimpleNormalizer.java
linguistics/src/main/java/com/yahoo/language/simple/SimpleTransformer.java
linguistics/src/main/java/com/yahoo/language/simple/kstem/

indexinglanguage/src/main/java/com/yahoo/vespa/indexinglanguage/linguistics/LinguisticsAnnotator.java
```

The verifier calls the public tokenizer/stemmer APIs directly. `LinguisticsAnnotator` is traced because its treatment of compound/indexable tokens defines how document-side tokenizer output is consumed by ordinary Vespa indexing; M4 will verify actual annotation/native-index integration end to end.
