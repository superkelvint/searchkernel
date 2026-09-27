# Static Search Kernel Native Runtime Spike

## Conclusion

**Candidate C/current model recommended.** Candidate B links and runs, but it
does not reduce the runtime closure on the pinned Vespa input: its 127 MiB
adapter still needs 18 Vespa DSOs and a large private DSO closure. Its expanded
payload was indistinguishable from Candidate C (1.29 GB). Candidate A could not
produce a static public-Rust executable through the current supported bindings:
the static-CRT attempt fails while compiling procedural macros, before a final
native link is reached.

This is enough evidence to reject a production static migration based on the
currently produced artifacts. Do not start package-manager feasibility work for
A or B. Candidate C may proceed to that phase using its existing consumer-safe
runtime layout.

## Environment

- Search Kernel: `ad0e67e1` (spike branch parent)
- Pinned Vespa: `2300ef0412bdb76f227ae3b497c8ca5e73c5da4e`
- Target: `x86_64-unknown-linux-gnu`; Linux x86-64
- Compiler: GCC 13; linker: GNU ld
- Native input: immutable `devcontainer` SDK base for the pinned Vespa SHA

## Results

| Metric | A: static-ish executable | B: mostly-static runtime | C: shared closure |
|---|---:|---:|---:|
| Public API smoke | not built | PASS with full closure | PASS |
| Intended minimal clean runtime | not applicable | FAIL: `libvespa_persistence.so` missing | not independently valid on this host* |
| Executable | — | 9.0 MiB | 9.0 MiB |
| Runtime library | — | 127.1 MiB | 127.1 MiB |
| Expanded native payload | — | 1.29 GB | 1.29 GB |
| Bundled Vespa DSOs | — | 18 | 18 |
| Native link wall time | — | 63.34 s | 71.11 s |
| Peak linker RSS | unavailable (`/usr/bin/time` absent) | unavailable | unavailable |
| Cargo duplication | not measurable | one 127 MiB runtime can be shared | one 127 MiB runtime can be shared |

\* The direct C smoke correctly executed from the staged tree, but its adapter
retained an absolute development-SDK RPATH because this host lacks `patchelf`.
The spike marks that as invalid clean-runtime evidence rather than claiming a
false pass. The supported release packager normally rewrites this to `$ORIGIN`.

## Candidate A

The supported bindings declare `dylib=searchkernel_vespa`. The spike attempted
the public smoke with `-C target-feature=+crt-static`; Cargo failed while
building `serde_derive` because this GNU target cannot produce its proc-macro
crate type with that setting. Consequently it never reached a final executable
link that could absorb the adapter or Vespa archives. A second Cargo target was
therefore not useful for payload comparison.

This is a real integration blocker, not an estimate: supporting A requires a
dedicated static-bindings build mode and a Rust toolchain/target strategy for
host proc macros. Even that would still have to solve the dynamic-only Vespa
and third-party inputs listed below.

## Candidate B

The spike rebuilt `libsearchkernel_vespa.so.2` with the pinned adapter build
script and the exact configured embedded-CRUD link order. It absorbed the
available Searchcore archives and preserved the existing version-script export
boundary. No `--whole-archive`, `--start-group`, or `--end-group` was needed:
the normal archive order linked and the full public API smoke passed, including
close/reopen.

The intended `application + libsearchkernel_vespa.so` layout failed cleanly
because `libvespa_persistence.so` was absent. Adding the complete dynamic
closure made the smoke pass, but produced the same 1.29 GB payload and 18
Vespa DSOs as Candidate C.

## Baseline C

Candidate C uses the normal Cargo-built adapter plus the repository runtime
closure bundler. Its staged smoke exercised create/open, two upserts, term and
phrase search, stored-field retrieval, close, reopen, and a second search.
The smoke passed. Its physical closure contains 121 regular native files,
including 18 Vespa DSOs.

## Static-link findings

The configured Vespa build supplies static Searchcore archives, including
`searchcore_server`, `searchcore_matching`, `searchcore_attribute`,
`searchcorespi`, persistence-engine, document-meta-store, index, grouping, and
related archives. The active published SDK deliberately ships no `.a` files.

The B link proves those Searchcore archives are PIC-compatible and usable in
the adapter without special registration retention flags for this smoke. The
remaining direct dynamic dependencies include `libvespa_searchlib.so`,
`libvespaeval.so`, `libvespalib.so`, persistence/document/config/messagebus
libraries, LLVM, ONNX Runtime, OpenBLAS, ICU, Protobuf, OpenSSL, RE2, Zstd,
and many Abseil DSOs. These dominate the delivered closure.

## Recommendation

Keep the relocatable shared closure for M1 package-manager prototypes. It is
the only model with a supported consumer layout today; Candidate B did not
remove enough DSOs or bytes to justify a new distribution model, and Candidate
A needs additional Rust/native build architecture before it can be evaluated
fairly.

Revisit B only if the pinned Vespa build can provide static forms of its major
shared libraries and private dependencies. At that point, rerun this spike
before changing production packaging.

## Reproduce

```bash
./dev native-sdk ensure
spikes/static-searchkernel/spike.sh all
```

Raw ELF output, linker output, and clean-runtime logs are generated below the
ignored `spikes/static-searchkernel/build/` directory.
