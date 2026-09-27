# Static Search Kernel Spike

This standalone M1 experiment determines whether Search Kernel should distribute
its embedded Vespa implementation as:

1. a static-ish final Rust executable;
2. one mostly-static Search Kernel runtime library; or
3. the current relocatable shared closure.

It does not alter production runtime packaging, CI, release policy, or the
pinned Vespa input.

## Run

From the repository root after the normal native SDK prerequisite is available:

```bash
./dev native-sdk ensure
spikes/static-searchkernel/spike.sh clean
spikes/static-searchkernel/spike.sh all
```

Individual candidates are:

```bash
spikes/static-searchkernel/spike.sh candidate-a
spikes/static-searchkernel/spike.sh candidate-b
spikes/static-searchkernel/spike.sh candidate-c
```

Generated binaries and diagnostics stay below the ignored
`spikes/static-searchkernel/build/` directory.

## Candidate A

Candidate A is **not** a static-CRT experiment.

The spike creates a disposable scratch copy of the Rust workspace and replaces
only the copied `searchkernel-vespa-bindings/build.rs` with a no-op so the
production `dylib=searchkernel_vespa` declaration does not predetermine the
answer.

A spike-only linker wrapper then injects the staticized Vespa/Search Kernel link
plan into the final Rust executable. Host proc macros and ordinary Cargo links
continue to use the normal linker.

System libc/libstdc++ and genuinely unavoidable platform libraries may remain
dynamic. The question is whether Search Kernel/Vespa-specific native code can be
absorbed into each final executable and what that costs.

## Candidate B

Candidate B is deliberately different from the production adapter.

The spike reads Vespa's configured embedded-CRUD link command and indexes the
static archives exposed by the pinned Vespa/dependency build. For every dynamic
link input it substitutes a matching `.a` only when the choice is
unambiguous, records the replacement, and links a fresh
`libsearchkernel_vespa.so.2`.

Evidence is written to:

```text
build/static-plan/results/static-replacements.tsv
build/static-plan/results/unresolved-static-candidates.tsv
build/static-plan/results/candidate-b-link.log
build/static-plan/results/candidate-b-link-metrics.json
```

If a major Vespa/private library has no usable static archive in the current
pinned artifact topology, that is recorded as a blocker rather than hidden by
falling back and calling the result "mostly static."

## Candidate C

Candidate C remains the current supported consumer-safe Cargo adapter plus
relocatable shared closure. It is the baseline.

## Smoke

All viable candidates use the same public Rust API smoke:

- create/open index;
- upsert documents;
- term search;
- phrase search;
- stored fields;
- close;
- reopen;
- search again.

Clean-runtime probes execute outside the repository with native environment
overrides removed.
