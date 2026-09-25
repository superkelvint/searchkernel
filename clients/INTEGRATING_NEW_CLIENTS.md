# Integrating New Search Kernel Clients

**Status:** Normative integration SOP  
**Audience:** Agents and engineers promoting an implemented non-Rust SDK into the repository's build, verification, compatibility, and release harnesses  
**Applies after:** `clients/IMPLEMENTING_NEW_CLIENTS.md` has produced a working client  
**Does not replace:** the implementation SOP, the canonical API/IR contracts, or language-specific packaging documentation

---

## 1. Purpose

`IMPLEMENTING_NEW_CLIENTS.md` explains how to build a correct client.

This document explains the next step: how to make that client a **first-class repository surface** that is continuously built, regenerated, checked for semantic compatibility, exercised through real E2E paths, represented in capability reports, and impossible to accidentally leave out of normal verification.

A client is not integrated merely because:

- its own tests pass;
- its package builds;
- an example can talk to `searchkerneld`;
- it has copied the Python or TypeScript public API;
- it appears in `clients/`.

Integration is complete only when the repository's shared harnesses know the client exists and will fail when it drifts.

The target state is:

```text
implemented client
    ↓
registered repository surface
    ↓
normal dev commands build/test/regenerate it
    ↓
shared conformance evidence includes it
    ↓
real E2E evidence is executable in CI
    ↓
generated compatibility/capability reports include it
    ↓
future protocol/client changes cannot silently skip it
```

The central rule is:

> **A first-class client must be part of the repository's default verification graph, not a side project with a README saying it works.**

---

# Part I — Preconditions

## 2. Do not integrate an unfinished architecture

Before starting this SOP, verify the client already satisfies the applicable implementation gates in `IMPLEMENTING_NEW_CLIENTS.md`.

At minimum, the client should already have:

- one explicitly sanctioned representation/transport path from `IMPLEMENTING_NEW_CLIENTS.md`;
- for binary gRPC/gRPC-Web clients, generated FlatBuffers models from the canonical `.fbs` and standard generated RPC plumbing from `api/rpc/searchkernel.proto`;
- for explicitly designated HTTP-first clients, standard FlatBuffers JSON envelope/value/request handling mechanically checked through the official BFBS-backed parser/printer;
- a supported low-level canonical escape hatch appropriate to that path;
- request/response envelope handling;
- portable value encoding/decoding;
- portable error handling;
- the intended ergonomic facade;
- result decoding;
- package/build metadata;
- language-local tests;
- managed-local lifecycle where applicable;
- at least one representative real `searchkerneld` path where the runtime supports it.

Do not use repository integration as a substitute for unfinished client implementation.

If the client needs changes to canonical semantics, `searchkernel-ir`, `searchkernel-dispatch`, the portable FlatBuffers contract, the generic transport edges, or Vespa lowering in order to integrate, stop and resolve that as a separate architectural or correctness change. A client must not redefine those layers merely to fit its host runtime.

---

## 3. Read the current integration surfaces before editing

Inspect the current versions of:

```text
clients/README.md
clients/IMPLEMENTING_NEW_CLIENTS.md
dev
docs/capabilities.json
reference/client-contracts/README.md
reference/client-contracts/catalog.json
tools/check-capabilities.py
tools/check-client-contracts.py
tools/test_check_capabilities.py
tools/test_check_client_contracts.py
.github/workflows/client-capabilities.yml
```

Also inspect:

- the current Python integration;
- the current TypeScript integration;
- all workflow files that mention either existing client;
- generation scripts for existing clients;
- package/install verification for existing clients;
- current shared result/conformance fixtures used by the client.

Do not assume these integration points are unchanged from a previous client addition.

---

# Part II — Register the Client as a Repository Surface

## 4. Add the client to the client-contract catalog

The canonical client capability registry is:

```text
reference/client-contracts/catalog.json
```

Add one `surfaces.<client>` entry with:

```json
{
  "title": "Go",
  "runtimes": ["grpc", "managed-local"],
  "evidence_roots": [
    "clients/go/tests",
    "clients/go/examples"
  ]
}
```

Use runtime names that reflect real execution environments, not marketing labels.

Examples:

```text
grpc
grpc-web
http
managed-local
shared
node
browser
jvm
native
```

The runtime entry is evidence metadata, not permission to invent a transport. A
first-party client using `http` must already be explicitly designated
HTTP-first by the architecture/SOP and must use the standard FlatBuffers JSON
generic execute edge.

Only list an evidence root if it exists and is intended to contain executable evidence.

Do not add positive support booleans to the catalog. Positive support comes from executable `SKC:` annotations.

---

## 5. Register real E2E lanes before claiming E2E evidence

`tools/check-client-contracts.py` only accepts `kinds=e2e` evidence when the exact evidence file/runtime is covered by a registered executable E2E lane.

Add `e2e_lanes` to the client surface when real E2E evidence exists:

```json
{
  "title": "Go",
  "runtimes": ["grpc", "managed-local"],
  "evidence_roots": [
    "clients/go/tests",
    "clients/go/examples"
  ],
  "e2e_lanes": [
    {
      "runtimes": ["managed-local"],
      "path": "clients/go/examples/try_searchkernel_client_api_e2e.go",
      "command": ["go", "test", "./clients/go/e2e", "-run", "TestClientAPI"]
    }
  ]
}
```

The command must be runnable from the repository root.

The registered lane is part of the evidence chain:

```text
SKC kinds=e2e annotation
        ↓
catalog e2e_lanes registration
        ↓
tools/check-client-contracts.py --run-e2e
        ↓
CI execution
```

If any link is missing, do not claim E2E.

### False-green trap

An example file containing `SKC: ... kinds=e2e` is not evidence merely because a human can run it.

The repository must execute the registered lane.

---

# Part III — Wire the Developer Command Surface

## 6. Add a first-class `./dev test <client>` command

Update `dev` so normal development has one canonical client test command.

Follow the existing pattern:

```text
./dev test python
./dev test typescript
./dev test <new-client>
```

Add a dedicated helper such as:

```text
run_go_tests()
run_java_tests()
run_php_tests()
```

That helper should run the language's complete ordinary verification set, including as applicable:

- unit tests;
- compile/type checks;
- generated-code checks;
- package-consumer tests;
- transport tests;
- language-specific lint/static analysis.

Do not make developers remember a bespoke command sequence that CI knows but `./dev` does not.

---

## 7. Add generation to the canonical regeneration path

If the client checks in generated FlatBuffers or gRPC artifacts, wire generation into:

```text
./dev regen protocol
```

The existing regeneration path owns canonical protocol-derived client artifacts
for clients that have generated trees.

A protocol regeneration must not update existing generated clients while
silently leaving another generated first-class client stale. An explicitly
HTTP-first client with no generated language tree is not forced into
regeneration; its BFBS-backed standard-JSON conformance check is the mechanical
schema-drift authority instead.

Where generated artifacts exist, requirements are:

- use the pinned toolchain;
- replace generated trees atomically;
- remove stale files;
- make regeneration deterministic;
- provide a check mode that fails on diff.

If a binary client deliberately does not check in generated code, document that
exception and ensure its build/test path generates from canonical source every
time. If an HTTP-first client deliberately has no generated language model/RPC
tree, document that representation choice and ensure its conformance path
reaches the official BFBS bridge every time.

---

## 8. Add cheap drift checks to `check-fast`

If the client has a cheap deterministic generated-artifact or boundary check, include it in:

```text
./dev check-fast
```

Good `check-fast` candidates:

- generated-model freshness where generated models exist;
- generated gRPC/gRPC-Web freshness where that transport is used;
- BFBS-backed standard-FlatBuffers-JSON conformance for HTTP-first clients;
- package manifest consistency;
- source-boundary checks;
- capability-report freshness.

Do not put expensive real-engine E2E into `check-fast`.

The goal is to catch repository drift before a full test cycle.

---

## 9. Add the client to `./dev check`

The full repository check must exercise the first-class client.

Wire the client's ordinary test helper into the full `check_all` path.

A client that is omitted from `./dev check` is not fully integrated, even if a separate workflow happens to test it.

---

## 10. Extend `./dev doctor` only for real shared prerequisites

Add doctor checks when the new client introduces a repository-level prerequisite that developers are expected to have for normal work.

Examples:

- required package-manager dependencies;
- generated tool binary that must exist;
- client-specific compiler/runtime needed by the standard test path.

Do not make optional platform/runtime support a mandatory doctor failure.

Prefer:

```text
doctor_check     required for normal first-class verification
doctor_optional  only needed for a specialized lane
```

The doctor message must say exactly how to install or populate the missing prerequisite.

---

# Part IV — Protocol Derivation and Build Integration

## 11. Add explicit generation/conformance checks

Use repository-owned scripts under `tools/` or the client test harness for deterministic protocol derivation and verification.

For binary/gRPC clients, prefer the existing naming pattern:

```text
tools/generate-<client>-flatbuffers.sh
tools/generate-<client>-grpc.sh
tools/check-<client>-generated.sh
tools/test-<client>-generation.sh
```

For explicitly HTTP-first clients, the equivalent required responsibility is a
mechanical BFBS conformance check over representative client-emitted and
client-consumed standard FlatBuffers JSON. Do not add unused generated RPC code
only to satisfy a filename convention.

Where generated artifacts exist, generation scripts must:

1. resolve the pinned tool;
2. verify its version;
3. generate from canonical schemas only;
4. write to a temporary directory;
5. replace the generated tree atomically;
6. fail closed on unsupported generation state.

Where standard FlatBuffers JSON is the selected low-level representation, the
conformance harness must fail closed on schema/discriminator/scalar-width drift
and malformed unions.

Do not hand-edit generated output or maintain a handwritten parallel JSON domain schema.

---

## 12. Make build provenance visible

The client package should make it obvious which canonical inputs define its wire representation:

```text
api/searchkernel.fbs
api/searchkernel_request.fbs
api/searchkernel_response.fbs
api/rpc/searchkernel.proto        # only when the client uses gRPC/gRPC-Web
generated BFBS                    # schema authority for standard FlatBuffers JSON
```

If additional schema entrypoints become canonical later, use the current repository list rather than freezing this document's examples forever.

The important invariant is that every client's low-level wire representation is mechanically derived from the same portable contract, while transport-specific framing is used only by clients that actually select that transport.

---

# Part V — Shared Compatibility and Capability Harnesses

## 13. Add `SKC:` evidence to executable tests, not implementation files

Positive client capability evidence is colocated with the executable test/example that proves it.

Format:

```text
SKC: <capability>/<scenario> kinds=<kind[,kind...]> runtime=<runtime>
```

Examples:

```text
// SKC: search.pagination/basic kinds=ergonomic,conformance runtime=shared
// SKC: update.document_crud/basic kinds=ergonomic,e2e runtime=managed-local
```

Kinds mean:

- `ergonomic` — the handwritten facade exposes the behavior;
- `conformance` — shared canonical semantic/contract fixtures exercise it;
- `e2e` — the behavior reaches the real product/backend path.

Do not use an ergonomic unit test as E2E evidence.

Do not use a real low-level transport test as proof that the ergonomic facade exposes the feature.

---

## 14. Port shared conformance fixtures rather than inventing client-specific expectations

When an existing canonical corpus exists, make the new client consume the same corpus.

Typical shared contracts include:

- portable request/response fixtures;
- result-model fixtures;
- filter semantics;
- presence-sensitive absent vs present-empty cases;
- numeric boundary cases;
- malformed union/envelope cases;
- canonical error cases;
- grouping/result-tree cases.

The acceptance target is semantic equivalence.

Do not create a second set of expected values merely because the host language has different syntax.

Host-language adaptation belongs at the assertion boundary:

```text
canonical int64 -> Python int
canonical int64 -> Java long / BigInteger policy
canonical int64 -> TypeScript bigint
```

The canonical meaning remains the same.

---

## 15. Update structured facade tracking where the global capability ledger uses it

`docs/capabilities.json` is the global represented/semantic/executable ledger.

Some capabilities also contain structured `facade_support` entries. These are validated by `tools/check-capabilities.py`.

When the new client becomes a tracked facade surface:

1. extend the checker surface/title registry as required;
2. add supported or missing facade state only where `facade_support` is part of the capability contract;
3. point supported state at real implementation symbols;
4. use absence evidence for intentionally missing facade support.

Do not mark a facade supported without executable ergonomic evidence. `tools/check-client-contracts.py` cross-checks those claims.

### Current hard-coded integration point

At the time this SOP was introduced, `tools/check-capabilities.py` contains an explicit facade-surface registry for the existing clients.

A new first-class facade must not be omitted from that registry merely because the client-contract catalog already knows about it.

Inspect the current checker instead of assuming the hard-coded list still has the same shape.

---

## 16. Regenerate compatibility reports

After adding evidence or changing tracked facade state, regenerate the generated reports:

```bash
python3 tools/check-capabilities.py --write
python3 tools/check-client-contracts.py --write
```

This may update:

```text
docs/CAPABILITIES.md
LIMITATIONS.md
docs/CLIENT_CAPABILITIES.md
docs/client-capabilities.json
```

Do not hand-edit generated reports.

Then prove they are current:

```bash
python3 tools/check-capabilities.py --check
python3 tools/check-client-contracts.py --check
```

---

# Part VI — CI Integration

## 17. Add toolchain/bootstrap steps to CI

A first-class client must be runnable on the repository's CI runners from a clean checkout.

Update the applicable workflow(s) to install or select:

- the language runtime/compiler;
- package manager;
- client dependencies;
- generation tools;
- any required isolated environment.

Use repository-standard runner selection and dependency-isolation patterns.

Do not rely on packages that happened to be installed manually on one self-hosted runner.

If a runner dependency is intended to be preinstalled, make `./dev doctor` and runner bootstrap documentation agree about it.

---

## 18. Wire the client into the client-capabilities workflow

The shared client workflow is:

```text
.github/workflows/client-capabilities.yml
```

The workflow already watches `clients/**`, but path triggering is only the first layer.

For a new client, ensure the workflow can actually run its registered evidence:

1. install/select the client toolchain;
2. install the package dependencies;
3. build/generate prerequisites if needed;
4. make every catalog `e2e_lanes.command` executable;
5. keep `tools/check-client-contracts.py --check` green;
6. keep `tools/check-client-contracts.py --run-e2e` green when E2E is claimed.

If the client's E2E requires a capability not available in ordinary GitHub CI, do not fabricate E2E support.

Either:

- provide an appropriate executable self-hosted lane;
- register a separate required workflow that executes it; or
- leave E2E unclaimed and document the blocker.

---

## 19. Inspect every workflow that special-cases existing clients

Search `.github/workflows/**` and `.github/ci/**` for:

```text
python
typescript
clients/python
clients/typescript
generate-python
generate-typescript
```

Classify each occurrence:

```text
must generalize/add new client
language-specific by design
obsolete and should be removed separately
```

Common places that may need integration:

- generated-artifact checks;
- client unit-test jobs;
- package/install jobs;
- capability jobs;
- dependency-policy checks;
- release/package workflows;
- cache configuration;
- workflow path filters.

Do not assume adding one job to `client-capabilities.yml` wires the client everywhere.

---

## 20. Keep CI and `./dev` symmetric

For ordinary non-native client verification, a developer should be able to reproduce CI through repository commands.

The expected relationship is:

```text
CI job
    ↓
repository script / ./dev command
    ↓
same test/build/generation logic
```

Avoid duplicating long language-specific command sequences directly in YAML when a repository script can own them.

A CI-only command and a local-only command are drift risks.

---

# Part VII — E2E and Real-Product Acceptance

## 21. Add the canonical public-API E2E

Every first-class client should have the language-equivalent public API showcase described in `IMPLEMENTING_NEW_CLIENTS.md`.

The E2E must use the public package/facade that users are expected to use.

It should not bypass the client through generated request objects except for explicit low-level API tests.

For managed-local runtimes, the E2E should prove:

```text
public Index.create/open
    ↓
managed searchkerneld lifecycle
    ↓
the client's sanctioned canonical transport path
    ↓
shared dispatcher / IR
    ↓
real backend
    ↓
close/reopen persistence
```

For remote/browser-only runtimes, the harness may provision the server externally, but the client portion must still use the public facade.

---

## 22. Separate three claims

Keep these distinct:

### A. Conformance

```text
client call
    ↓
canonical request/result meaning
```

May use recording/scripted/shared fixtures.

### B. Transport integration

Binary/gRPC example:

```text
canonical FlatBuffer bytes
    ↓
real generated gRPC/gRPC-Web path
    ↓
dispatcher
```

HTTP-first example:

```text
standard FlatBuffers JSON
    ↓
real generic HTTP execute path
    ↓
official BFBS-backed parser
    ↓
canonical FlatBuffer bytes
    ↓
dispatcher
```

Proves transport/representation plumbing, not necessarily real Vespa execution.

### C. Product E2E

```text
public facade
    ↓
real transport
    ↓
real dispatcher/IR
    ↓
real backend
    ↓
observable result/persistence
```

Do not label A or B as C.

---

## 23. E2E must fail for the right reasons

Before accepting a new E2E lane, ask how it could be falsely green.

Check specifically for:

- test accidentally using a fake/scripted backend;
- test spawning a daemon but asserting only serialization;
- server startup failure hidden by fallback behavior;
- test connecting to a pre-existing daemon;
- fixture-specific result generation;
- skipped tests treated as success;
- exit status ignored by wrapper script;
- example command that prints an error but exits zero;
- E2E annotation on a file different from the command actually run;
- persistence test reopening a different directory;
- browser test actually executing only under Node;
- TCP claim backed only by UDS;
- managed-local claim backed only by explicit remote connect.

The lane must prove the capability it advertises.

---

# Part VIII — Packaging and Consumer Integration

## 24. Verify the packaged artifact, not only the source tree

A first-class client must prove a clean consumer can use the package artifact.

The package test should verify, as applicable:

- main ergonomic import;
- low-level generated import;
- runtime-specific entrypoints;
- type declarations;
- generated files are present;
- native/runtime dependencies resolve;
- browser bundle succeeds when claimed;
- examples compile against the package artifact.

Do not let tests accidentally import source files that the published artifact omits.

---

## 25. Wire release/build metadata if the repository publishes the client

If the repository has release automation for first-party SDKs, add the new package to:

- versioning;
- artifact build;
- publish;
- changelog/release notes;
- package signing/provenance;
- release smoke tests.

If publication is intentionally deferred, say so explicitly. Do not silently leave a "first-class" client impossible to release.

---

# Part IX — Documentation Integration

## 26. Link both SOPs from the client README

`clients/README.md` should distinguish:

```text
IMPLEMENTING_NEW_CLIENTS.md
    how to build a client correctly

INTEGRATING_NEW_CLIENTS.md
    how to wire a completed client into repository-wide verification
```

A contributor adding a new client should encounter both documents without knowing their filenames in advance.

---

## 27. Update user-facing client inventories only from verified state

When the client is genuinely first-class, update any user-facing supported-language inventory.

Do not advertise a client as first-class before:

- ordinary repository checks include it;
- capability evidence includes it;
- package verification passes;
- required E2E is executable;
- generated compatibility reports are current.

Documentation follows verified integration state; it does not create it.

---

# Part X — Required Verification

## 28. Focused checks

Run the new client's canonical local test command:

```bash
./dev test <client>
```

Run generated/drift checks directly when debugging them.

Run the capability harness:

```bash
python3 tools/check-capabilities.py --check
python3 tools/check-client-contracts.py --check
python3 tools/test_check_capabilities.py
python3 tools/test_check_client_contracts.py
```

If the client claims E2E:

```bash
python3 tools/check-client-contracts.py --run-e2e
```

---

## 29. Repository checks

At minimum:

```bash
./dev check-fast
./dev check
```

If `./dev check` requires prerequisites unavailable in the current environment, follow `BUILDING.md` and report the exact unrun check. Do not call the integration complete because the client-local suite passed.

---

## 30. Clean-regeneration check

From a clean task branch/worktree:

1. run `./dev regen protocol`;
2. verify the intended generated client outputs are refreshed;
3. verify no unrelated generated drift appears;
4. rerun the client's generated-code check;
5. ensure a second regeneration produces no diff.

This proves the new client is actually part of the canonical generation graph.

---

## 31. Clean-consumer check

Build/package the SDK, then consume that artifact from outside its source directory.

Verify at least:

```text
import/load package
construct a representative request
load selected low-level canonical API
run package-level smoke test
```

Use the language's normal package-consumer mechanism.

---

# Part XI — Integration Review Checklist

## 32. Repository wiring

- [ ] Client exists under `clients/<language>/`.
- [ ] `clients/README.md` references the client where appropriate.
- [ ] `./dev test <client>` exists.
- [ ] `./dev check` runs the client.
- [ ] Cheap generated/drift or BFBS-conformance checks participate in `./dev check-fast`.
- [ ] `./dev regen protocol` refreshes checked-in generated artifacts for clients that have them; HTTP-first clients without generated trees are covered by their canonical BFBS conformance check.
- [ ] `./dev doctor` covers required shared prerequisites where appropriate.
- [ ] CI bootstraps the toolchain/dependencies from a clean checkout.
- [ ] CI and local repository commands share the same underlying scripts.

## 33. Capability/conformance wiring

- [ ] Client surface is registered in `reference/client-contracts/catalog.json`.
- [ ] Runtime names are explicit and accurate.
- [ ] Evidence roots contain executable tests/examples.
- [ ] `SKC:` annotations describe real evidence.
- [ ] Shared semantic fixtures are consumed where applicable.
- [ ] Global `facade_support` checker knows the new surface where applicable.
- [ ] Supported facade claims have structural and executable evidence.
- [ ] Missing/blocked support is explicit rather than silently absent.
- [ ] Generated capability reports are refreshed and clean.

## 34. E2E wiring

- [ ] Public-facade E2E exists.
- [ ] Every `kinds=e2e` annotation has an exact catalog E2E lane.
- [ ] Every catalog E2E lane is run by CI.
- [ ] E2E reaches the claimed real product/backend path.
- [ ] Managed-local E2E proves process lifecycle when claimed.
- [ ] Persistence close/reopen is verified when claimed.
- [ ] Browser E2E is actually browser execution when browser support is claimed.
- [ ] TCP/UDS claims are each backed by the corresponding transport path.

## 35. Packaging/release wiring

- [ ] The low-level canonical representation is present/reproducible for the selected path: generated models for binary clients, or supported standard-FlatBuffers-JSON helpers for HTTP-first clients.
- [ ] Ergonomic API is importable by an external consumer.
- [ ] The selected low-level canonical API/escape hatch is importable by an external consumer.
- [ ] Package tests consume the packed/built artifact.
- [ ] Runtime-specific entrypoints work.
- [ ] Release automation includes the client, or publication deferral is explicit.

## 36. Adversarial false-green review

- [ ] No test can pass by silently skipping the new client.
- [ ] No capability claim is derived from comments without executable coverage.
- [ ] No E2E claim points at an unexecuted file.
- [ ] No workflow tests only source-tree imports while package output is broken.
- [ ] No generated-code check ignores stale deleted files where generated artifacts exist.
- [ ] No protocol regeneration or BFBS conformance inventory omits the new client.
- [ ] No hard-coded list of existing clients still excludes the new one.
- [ ] No compatibility matrix requires manual edits to remain current.
- [ ] No client-specific defaults or semantic validation were introduced during integration.

---

# Part XII — Definition of Done

## 37. A client is integrated only when omission becomes a failure

The strongest test of this SOP is counterfactual:

> If the new client drifts, stops compiling, stops generating, loses a claimed capability, breaks package output, or its E2E stops reaching the real product path, does a normal repository gate fail?

If the answer is no, the integration is incomplete.

A fully integrated first-class client has:

```text
implementation
+
repository dev command
+
canonical generation path
+
shared conformance evidence
+
capability registration
+
executable E2E lane
+
CI bootstrap/execution
+
package consumer verification
+
generated compatibility reporting
+
adversarial false-green review
```

The implementation SOP makes the client correct.

This integration SOP makes that correctness durable.
