# M4 verifier authoring status

The direct C++ oracle is now authored. The package becomes implementation-ready
only after it has been run against the pinned Vespa checkout and the resulting
fixture/goldens have been frozen.

Before handing M4 to an implementation agent:

1. Ensure M3 is frozen and passes.
2. Ensure the pinned Vespa C++ build tree exists. If it is outside
   `upstream/vespa`, export `VESPA_BUILD_DIR=/path/to/build`.
3. Run:

   ```bash
   ./reference/vespa/m4/freeze-goldens.sh
   ```

4. Review `reference/vespa/m4/expected/results.jsonl`.
5. Confirm `M4-IMMUTABLE.sha256` exists and passes with `sha256sum -c`.
6. Commit the complete frozen harness.

Only after those steps should the M4 implementation agent receive
`verify-m4.sh` as immutable acceptance.

M4's behavioral authority is the direct embedded Vespa C++ engine. M3's Java
oracle is used only by verifier tooling to supply authoritative analyzed terms
and source spans to that direct C++ path.
