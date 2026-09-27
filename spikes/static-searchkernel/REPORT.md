# Static Search Kernel Native Runtime Spike

## Status

**Rerun required after the Candidate A/B methodology correction.**

The previous run is intentionally not treated as the M1 native-runtime decision:

- the old Candidate B reused the production adapter link topology and therefore
  was not sufficiently distinct from Candidate C;
- the old Candidate A used global Rust `+crt-static`, which tested static CRT
  behavior and failed in host proc-macro compilation before exercising the
  intended Search Kernel/Vespa executable-level native link.

The spike now implements distinct experiments:

- **Candidate A** builds a disposable scratch copy of the Rust workspace, removes
  only the copied bindings crate's production dylib declaration, and injects the
  staticized native link plan into the final Rust executable link. System libc
  and other ordinary platform runtime libraries may remain dynamic.
- **Candidate B** starts from Vespa's configured embedded-CRUD link topology and
  substitutes dynamic native inputs with matching static archives wherever the
  pinned Vespa/dependency build exposes one unambiguously. It then links a new
  `libsearchkernel_vespa.so.2`.
- **Candidate C** remains the current production consumer-safe shared-closure
  baseline.

Do not use the previous Candidate C recommendation as architectural evidence.
Rerun the corrected spike and replace this status section with measured results.

## Rerun

From the spike branch:

```bash
./dev native-sdk ensure
spikes/static-searchkernel/spike.sh clean
spikes/static-searchkernel/spike.sh all
```

The corrected run writes evidence below:

```text
spikes/static-searchkernel/build/results/
```

Important files include:

```text
static-plan/results/static-replacements.tsv
static-plan/results/unresolved-static-candidates.tsv
static-plan/results/candidate-b-link.log
static-plan/results/candidate-b-link-metrics.json
candidate-a/build.log
candidate-a/final-link-command.txt
candidate-a/clean-runtime.txt
candidate-a-closure/clean-runtime.txt
candidate-b/...
candidate-b-minimal/clean-runtime.txt
candidate-b-closure/clean-runtime.txt
candidate-c/...
```

## Report structure after rerun

Replace this temporary status with the final evidence-backed report:

### Conclusion

Choose one:

- Candidate A recommended;
- Candidate B recommended;
- Candidate C/current model recommended;
- insufficient evidence.

### Environment

Record:

- Search Kernel SHA;
- pinned Vespa SHA;
- target/platform;
- compiler/linker;
- native input identity.

### Results

| Metric | A: static-ish executable | B: mostly-static runtime | C: shared closure |
|---|---:|---:|---:|
| Public API smoke | | | |
| Minimal clean runtime | | | |
| Full required runtime | | | |
| Executable bytes | | | |
| Runtime library bytes | | | |
| Expanded payload bytes | | | |
| Remaining Vespa/private DSOs | | | |
| Native link wall time | | | |
| Peak linker RSS | | | |
| Cargo duplication | | | |

### Candidate A

Document:

- final executable link result;
- which native archives were absorbed;
- remaining dynamic dependencies;
- clean-runtime result;
- second Cargo target duplication.

### Candidate B

Document:

- every DSO→archive replacement from `static-replacements.tsv`;
- every unresolved/ambiguous static candidate;
- whether the new shared runtime links;
- remaining DT_NEEDED entries;
- minimal/full-closure smoke results.

### Candidate C

Record the current baseline using the same measurements.

### Static-link findings

State clearly:

- what the configured pinned Vespa build already exposes as static archives;
- which major native inputs have no usable static archive in the current
  artifact topology;
- whether archive order/grouping/whole-archive treatment was necessary;
- which dynamic dependencies dominate the remaining payload.

### Recommendation

Recommend the model that should proceed to the Cargo/pip/npm/RubyGems
feasibility prototypes required by M1.

Do not recommend A or B merely because they link. Prefer correctness,
relocatability, package practicality, size, and build behavior in that order.
