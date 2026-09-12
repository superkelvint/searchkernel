# KStem parity verifier

This verifier proves that Search Kernel's Rust KStem implementation matches the
pinned Vespa `com.yahoo.language.simple.kstem.KStemmer` for a large deterministic
corpus.

## Authority

The oracle is the pinned Vespa Java source, compiled directly from:

```text
linguistics/src/main/java/com/yahoo/language/simple/kstem/
```

No Java KStem behavior is reimplemented by the verifier.

## Corpus

The frozen corpus contains:

- every lowercase ASCII string literal present in the pinned Vespa KStem package,
  including dictionary, exception, conflation, nationality, supplement and proper
  noun data;
- systematic suffix mutations for every source seed to exercise the KStem suffix
  machinery;
- orthographic variants for `y`, terminal `e`, `-ion`/`-ive`, `-ize`/`-ization`,
  and doubled-consonant paths;
- length, case, numeric, punctuation and non-ASCII boundary cases.

The corpus generator does not encode expected stems. Vespa alone creates the
expected table.

## Freeze once

A maintainer with the pinned Vespa checkout runs:

```bash
VESPA_SRC=/work/upstream/vespa ./reference/vespa/m3/kstem/freeze-kstem.sh
```

Commit:

```text
frozen/corpus.txt
frozen/expected.tsv
frozen/PINNED_SOURCE.txt
frozen/FROZEN.sha256
```

The implementation agent must not regenerate these files.

## Verify

Ordinary verification requires only Rust + Python:

```bash
./reference/vespa/m3/kstem/verify-kstem.sh
```

It compiles the actual checked-in `stemmer.rs` and `data.rs` directly, avoiding
Cargo/native-SDK coupling, and compares every result to the frozen Vespa golden.

To additionally prove the golden still matches the pinned Java source:

```bash
KSTEM_VERIFY_JAVA_ORACLE=1 \
VESPA_SRC=/work/upstream/vespa \
./reference/vespa/m3/kstem/verify-kstem.sh
```

## Failure semantics

Any input whose Rust stem differs from Vespa is a failure. There are no tolerated
or implementation-defined KStem differences in the supported input domain.
