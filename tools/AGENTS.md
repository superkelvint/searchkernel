# Tooling agent rules

These instructions apply to everything under `tools/` and supplement the repository-root
`AGENTS.md`. Tooling is part of the product's build and verification contract. A helper
script that happens to work in one shell or one CI image is not acceptable.

## Absolute rules

### Never bypass the canonical toolchain authorities

Repository tooling MUST resolve language/runtime tools through the repository authorities
defined in `BUILDING.md`.

For Python:

- shell scripts must resolve Python with `tools/resolve-python.sh`;
- Python code that needs to launch another Python process MUST use `sys.executable`;
- NEVER invoke bare `python` or `python3` from repository tooling;
- NEVER assume `/usr/bin/python3`, `/usr/local/bin/python3`, a virtualenv path, or the
  first interpreter on `PATH` is the repository Python;
- NEVER "fix" an interpreter mismatch by installing the missing package into whichever
  Python happened to fail.

The same rule applies to Rust, Node/npm, FlatBuffers, and other multi-install toolchains:
use the canonical resolver instead of a bare executable.

A parent process choosing the correct interpreter does not make a nested bare executable
safe. If a Python script was launched by the canonical Python and later runs
`subprocess.run(["python3", ...])`, that is a bug. Use `sys.executable`.

### Never put Python programs in shell heredocs

NEVER embed executable Python in a shell heredoc, including forms such as:

```sh
python3 - <<'PY'
...
PY
```

or:

```sh
"$PYTHON" - <<'PY'
...
PY
```

If logic is substantial enough to require Python, put it in a tracked `.py` file with
tests and invoke that file through the canonical Python authority.

Reasons are practical, not stylistic: heredoc programs evade normal Python syntax checks,
unit-test discovery, import reuse, code ownership, static analysis, and toolchain guards.
They also make shell quoting and failure reporting harder to reason about.

For tiny data extraction, prefer an existing tracked helper. If none exists, add one.
Do not create a new inline Python heredoc as a shortcut.

### Do not make CI depend on an image change that the current CI run cannot see

Search Kernel CI executes a published immutable image. Editing a Dockerfile does not
retroactively add a package or executable to the image running the current pull request.

Before relying on a new image dependency:

1. determine whether the current verification lane actually rebuilds/publishes that image;
2. if it does not, do not write a source change whose correctness depends on the unpublished
   image;
3. make the repository code correct against the currently declared environment, or follow
   the explicit image-publication workflow when the dependency genuinely belongs in the
   image.

Never paper over a source-level tool-selection bug by adding a package to the CI image.

### Do not install dependencies ad hoc from tooling or CI lanes

Normal repository checks must not run opportunistic `pip install`, `npm install`,
`apt install`, `dnf install`, `cargo install`, or equivalent commands just to make a
lane pass.

Dependencies belong in their declared, reproducible authority:

- the immutable CI/toolchain image;
- the checked-in package manifest and lockfile;
- the documented bootstrap/doctor path;
- another explicit repository-owned provisioning mechanism.

If a dependency is missing, fix the declaration/provisioning contract. Do not mutate the
runner until the failure disappears.

### Never hand-edit generated artifacts

Files marked generated, generated protocol bindings, packaged source bundles, fixtures with
an authoritative generator, and other generated outputs must be regenerated with their
canonical generator.

When a source file included in a generated bundle changes, regenerate the bundle in the
same change. Run the corresponding `--check`/reproducibility verifier before handoff.

Do not weaken or exclude a generated-artifact check merely because regeneration was
forgotten.

## Tooling conventions

### Respect repository output roots

Do not assume Cargo writes to `target/`. Respect `CARGO_TARGET_DIR`; the repository also
uses `target/host` outside CI.

Likewise, do not assume build products live at conventional paths when the authoritative
tool exposes a target/output directory. Resolve or derive the path from the repository
contract instead of hardcoding a developer-machine layout.

### Keep shell thin

Shell should primarily orchestrate tracked programs and commands. Put non-trivial parsing,
state machines, JSON transformation, validation, or business logic in a real source file
that can be unit tested.

For Bash scripts:

- use `set -euo pipefail` unless the script has a documented reason not to;
- quote variable expansions and paths;
- use arrays when constructing commands with dynamic arguments;
- do not use `eval`;
- make temporary state job/process-private and clean it up;
- preserve the original failing exit status when cleanup runs;
- emit actionable errors to stderr.

Do not hide failures with broad `|| true`. Limit tolerated failures to the exact command
whose failure is intentionally non-fatal.

### Prefer portable baseline tools in critical scripts

Critical bootstrap, doctor, verification, and recovery scripts must not casually depend on
convenience binaries that may be absent on a fresh machine or reduced CI environment.

- For simple recursive/text searches, prefer `grep` over `rg`.
- Do not add a hard dependency on `rg`, `jq`, or another convenience CLI merely to save a
  few lines of code unless that dependency is explicitly part of the documented bootstrap
  and is verified before the script reaches it.
- If structured processing genuinely needs Python, use a tracked Python helper invoked via
  the canonical Python authority; do not replace the missing CLI with a Python heredoc.
- A verifier whose first failure is `command not found` for an undeclared convenience tool
  is a tooling bug.

GitHub CLI usage must likewise respect the supported developer baseline. Do not assume the
latest `gh` release or use newer flags without either a documented minimum-version contract
and doctor check or a compatibility path. In particular, avoid designing bootstrap/recovery
logic around optional convenience flags when the same operation can be implemented with
older stable `gh api` primitives or a tracked helper.

### Do not mutate shared or immutable build state as a repair tactic

Follow `BUILDING.md`. In particular, do not use `cargo clean`, delete `target/`, mutate
published native SDK/cache objects, copy SDK directories between machines, or rebuild
pinned Vespa as an ad-hoc response to a failing check.

Fix provenance, isolation, resolver, or provisioning bugs at their source.

### Do not write into `upstream/vespa`

The pinned Vespa tree is an upstream authority and must remain read-only unless a task
explicitly says otherwise. Search Kernel fixes belong in Search Kernel-owned adapters,
tooling, fixtures, or pinned-version updates.

### Fail closed

Verification and packaging scripts should reject inconsistent state rather than silently
guessing, skipping, falling back, or manufacturing defaults.

Examples:

- missing required corpus -> fail, unless an explicitly documented orchestrator owns that
  test and the non-orchestrated test is intentionally skipped;
- stale generated output -> fail and name the generator command;
- wrong interpreter/toolchain -> fail and name the canonical resolver/doctor path;
- missing artifact -> fail with the expected authoritative location;
- unsupported platform/combination -> report it as unsupported, do not approximate.

A skip is acceptable only when ownership is explicit and another named lane exercises the
behavior. Add regression coverage proving both the skip condition and the owning lane.

### Preserve exact-head and isolation guarantees

CI helpers must operate on the requested revision, not whichever checkout or mutable
workspace is nearby. Do not reuse another job's writable source tree or target directory.
Do not add fallbacks that can silently verify a stale head.

When editing CI/tooling, ask how the check could become falsely green.

### Regression-test tooling bugs

When fixing a tooling or CI defect, add the smallest regression that would have caught the
specific mistake whenever practical.

Important classes include:

- canonical-interpreter/tool resolver bypass;
- hardcoded output directory;
- stale generated bundle;
- missing dependency hidden by a developer machine;
- cross-job workspace leakage;
- exact-head verification drift;
- a verifier that can skip the code path it claims to prove.

Do not "fix" a failing verifier by weakening its assertion until it passes. First prove the
verifier expectation is wrong.

## Before handoff

For a tooling change, run the focused regression plus the repository checks appropriate to
the blast radius. At minimum consider:

```sh
bash tools/check-shell-syntax.sh
bash tools/test-shell-syntax.sh
./dev check-fast
```

Also run the specific generator `--check`, packaging check, CI regression, or client/native
lane affected by the change.

Do not claim success from a green run attached to an older commit. Verification evidence
must correspond to the exact head being handed off.
