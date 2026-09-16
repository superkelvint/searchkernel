# M10 frozen goldens

The eight `*.json` files in this directory are intentionally generated only by
`../freeze-goldens.sh` from the reviewed **direct pinned Vespa C++ oracle**.

Do not hand-author scores or ranking-feature values and do not run the Rust
implementation to create them. M10 compares exact IEEE-754 bit patterns, so a
plausible-looking handwritten score would defeat the purpose of the verifier.

Verifier-authoring sequence:

```bash
reference/vespa/m10/oracle/build-cpp-oracle.sh
# review the oracle source and a sample oracle run
reference/vespa/m10/freeze-goldens.sh
reference/vespa/m10/validate_expected.py
git add reference/vespa/m10/expected/*.json
```

Commit the resulting files with the verifier/specification commit **before**
implementing M10 production code.
