# M0 probe coverage

Run:

```bash
./tools/coverage-m0-probe.sh
```

The harness compiles `reference/vespa/m0/native-probe/probe.c` with GCC
coverage instrumentation and requires 100% line and branch coverage. It runs
the real packaged native library for the M0 open/close acceptance path. The
probe's native-failure branches are exercised with the small ABI double in
`tools/m0-probe-fake.c`; that double is test-only and is never used by the
acceptance path.

This measures the probe program, not the Vespa C++ adapter or the prebuilt
Vespa SDK. Native adapter coverage requires a separately instrumented native
build.
