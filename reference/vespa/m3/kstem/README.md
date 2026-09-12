# Installing this verifier

Copy this directory to:

```text
reference/vespa/m3/kstem/
```

Then freeze the oracle once against the project's pinned Vespa checkout:

```bash
VESPA_SRC=/work/upstream/vespa ./reference/vespa/m3/kstem/freeze-kstem.sh
```

Review and commit the generated `frozen/` files. From then on, acceptance runs:

```bash
./reference/vespa/m3/kstem/verify-kstem.sh
```

For CI/reviewer environments that also have the pinned Vespa source + JDK:

```bash
KSTEM_VERIFY_JAVA_ORACLE=1 ./reference/vespa/m3/kstem/verify-kstem.sh
```

The normal verifier does **not** regenerate goldens.
