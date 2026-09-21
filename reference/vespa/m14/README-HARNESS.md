# M14 verifier harness

M14 extends M12/M13 with a real HNSW-backed tensor attribute.

Author/reviewer sequence:

```bash
reference/vespa/m14/validate_scenarios.py
reference/vespa/m14/validate_expected.py
reference/vespa/m14/oracle/build-cpp-oracle.sh
reference/vespa/m14/freeze-goldens.sh   # verifier-authoring only
reference/vespa/m14/verify-m14.sh
```

`verify-m14.sh` never regenerates expected output.

The package deliberately separates:

1. generated configuration proof (`canonicalize_cfg.py`);
2. direct pinned-Vespa execution oracle;
3. low-level Rust execution;
4. portable FlatBuffers/IR/adapter execution;
5. structural anti-shortcut checks.

The implementation commit must not modify `expected/` or weaken the verifier.
