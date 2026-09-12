# M6 verifier workflow

This directory is the single M6 verifier. Replace the previous M6 verifier with these files; do not keep parallel verifier versions.

If M6 expected artifacts need to be updated during verifier authoring, remove
only the generated artifacts and regenerate them:

```bash
rm -f reference/vespa/m6/expected/*.json
rm -f reference/vespa/m6/scenarios/rendered/*.sd
./reference/vespa/m6/freeze-goldens.sh
```

Do **not** delete `reference/vespa/m6/oracle/target/`; it is generated Maven/oracle cache and keeping it avoids rebuilding the dependency closure unnecessarily.

`freeze-goldens.sh`:

1. verifies the pinned Vespa revision;
2. validates scenario invariants, including the identical-field `typea`/`products` pair;
3. renders equivalent `.sd` fixtures;
4. runs the real pinned Vespa Java config-model oracle;
5. writes `expected/*.json`;
6. validates expected outputs.

After review, commit the verifier specification. Implementation acceptance is:

```bash
./reference/vespa/m6/verify-m6.sh
```

The normal verifier compares Rust output to frozen expected semantics, optionally compares against the live Java oracle, validates the private document-schema manifest, and opens every generated directory through the real embedded engine.

Set:

```bash
M6_LIVE_ORACLE=0
```

to skip the redundant live Java comparison during fast local iteration. Frozen expected output remains mandatory. The default is `M6_LIVE_ORACLE=1` for final acceptance.
