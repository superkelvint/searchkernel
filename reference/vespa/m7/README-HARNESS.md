# M7 verifier harness

This directory follows the standardized verifier layout used by M3–M6.

## Authoring phase

Before M7 implementation begins:

```bash
./reference/vespa/m7/freeze-goldens.sh
```

This:

1. validates the scenario specification;
2. builds/runs the pinned Vespa Java YQL oracle;
3. writes canonical Java-derived query trees under `expected/`;
4. validates the expected corpus;
5. writes `M7-IMMUTABLE.sha256`.

Review `expected/*.json` before committing them.

Do not hand M7 to an implementation agent until `M7-IMMUTABLE.sha256` exists and `check-immutable.sh` passes.

## Implementation acceptance

After the verifier is frozen:

```bash
./reference/vespa/m7/verify-m7.sh
```

Acceptance is cumulative through M6 and then verifies:

```text
typed Query canonical shape
YQL -> Query canonical shape
live pinned Java YQL parity
field/type errors
real Engine execution for typed queries
real Engine execution for YQL
identical typed/YQL hit sets
```

The Java oracle is never part of production runtime.
