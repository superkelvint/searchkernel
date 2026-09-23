# 13. Daemon/RPC host — `searchkerneld`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)

**Threat model:** every TCP, gRPC-Web, HTTP/JSON, or future public-search listener is an untrusted-input boundary. Unix-domain sockets are less exposed but must still treat client bytes as hostile. A public search endpoint may intentionally be unauthenticated, but it must expose only the intended read-only search capability; generic daemon execution, mutation, administration, filesystem selection, and other privileged operations are separate capabilities and require explicit isolation/authentication.

**Minimum evidence to PASS this section:** `SRC + UNIT + PKG + NEG`; transport behavior requires real daemon execution. Publicly reachable request paths additionally require `E2E + HARD` evidence covering hostile payloads, fuzzing, saturation/load, cancellation, connection abuse, and resource-growth behavior. Backend search semantics are not proven here, but the audit must prove that every accepted request reaches the canonical semantic path and that the daemon cannot be used to bypass it.

## 13.1 Process/config lifecycle — `src/main.rs`, `src/lib.rs`

- [ ] **SK-13-001** CLI/config parsing in `main.rs` is included in the audit; argument defaults cannot create a semantic path that library-level daemon tests never exercise.
- [ ] **SK-13-002** Index create/open arguments cannot accidentally create when open was intended or vice versa.
- [ ] **SK-13-003** Definition is only required/accepted in the intended create path.
- [ ] **SK-13-004** Startup failure exits without leaving stale locks/sockets/resources.
- [ ] **SK-13-005** Shutdown drains/stops listeners and releases backend/native state in safe order.

## 13.2 gRPC service

- [ ] **SK-13-006** Server uses standard generated gRPC service code.
- [ ] **SK-13-007** `Execute` forwards raw frame payload to the shared dispatcher without operation-specific semantics.
- [ ] **SK-13-008** Empty/malformed/oversized frames fail deterministically.
- [ ] **SK-13-009** Portable Search Kernel errors remain response payloads; transport failures remain gRPC transport errors.
- [ ] **SK-13-010** Deadlines/cancellation do not corrupt backend state.
- [ ] **SK-13-011** Request/response size limits are explicit and tested.

## 13.3 Bounded execution

- [ ] **SK-13-012** Blocking native dispatch is executed off the async reactor.
- [ ] **SK-13-013** Concurrency semaphore/limit is honored under load.
- [ ] **SK-13-014** Cancellation while queued cannot leak a semaphore permit or allow a canceled request to start work later unexpectedly.
- [ ] **SK-13-015** Cancellation/deadline while `spawn_blocking` native execution is already running has a documented contract: dropping the RPC future does not pretend the native operation was canceled; backend state remains valid and the permit is released only when blocking work actually finishes.
- [ ] **SK-13-016** Overload behavior is bounded and does not create unbounded queues/threads.

## 13.4 Unix-domain sockets

- [ ] **SK-13-017** Existing live socket causes startup failure rather than unlinking another server.
- [ ] **SK-13-018** Only genuinely stale sockets are removed.
- [ ] **SK-13-019** Permission/transient/refused connection errors are not automatically treated as proof that an existing socket is stale; stale detection distinguishes ownership/liveness cases deliberately.
- [ ] **SK-13-020** Shutdown cleanup cannot unlink a replacement socket created by another process after handoff/race.
- [ ] **SK-13-021** Server tracks/validates ownership before removing socket path on normal shutdown **and Drop**; cleanup must not unlink a path that has been replaced by another process after bind.
- [ ] **SK-13-022** Non-socket file at configured path fails safely.

## 13.5 TCP/gRPC-Web/HTTP edge

- [ ] **SK-13-023** TCP listener exposes the same dispatcher semantics as UDS.
- [ ] **SK-13-024** Optional gRPC-Web and HTTP/JSON edge listeners share the same dispatcher, admission bound, and execution bound as the native gRPC path.
- [ ] **SK-13-025** gRPC-Web does not introduce domain semantics, and any HTTP/JSON edge uses only the official schema-aware FlatBuffers JSON bridge before the same canonical dispatcher; there is no handwritten JSON -> IR path.
- [ ] **SK-13-026** CORS, allowed methods, media types, preflight behavior, and HTTP error behavior are explicitly configured/tested where relevant; CORS/origin checks are never treated as authentication or authorization.

## 13.6 Generated RPC freshness

- [ ] **SK-13-027** Checked-in Rust RPC generated module matches fresh standard-tool generation.
- [ ] **SK-13-028** RPC generation is deterministic and pinned.

## 13.7 Public-search capability isolation and exposure

- [ ] **SK-13-029** Every network listener is classified as local-only, trusted-network, or public. Defaults remain UDS/loopback/local-safe; listening on a non-loopback interface is an explicit deployment choice rather than an accidental consequence of a broad default.
- [ ] **SK-13-030** A public search endpoint is **read-only by construction**. Prove that insert/upsert/patch/update/delete, create/open/admin operations, or any other privileged portable operation cannot be smuggled through the public route by changing a FlatBuffer discriminator, JSON union member, protobuf frame, content type, or alternate transport.
- [ ] **SK-13-031** The existing generic `Execute` capability is not assumed to be safe merely because the intended caller sends search requests. If generic execution is reachable from an untrusted network, it is protected by an explicit capability/authentication boundary; otherwise it is bound only to an appropriate trusted/local listener.
- [ ] **SK-13-032** Any operation allowlist/capability gate is narrow transport policy only: it may identify the verified operation class to reject disallowed capabilities, but it does not duplicate query/filter/ranking/grouping/default semantics owned by `*_to_ir()`.
- [ ] **SK-13-033** Public search cannot select arbitrary filesystem paths, index directories, Unix sockets, configuration files, summary/model assets, or other local resources from caller-controlled path strings. If public index selection exists, identifiers resolve through a fixed registry/namespace with traversal and aliasing tests.
- [ ] **SK-13-034** Plaintext public TCP exposure has an explicit deployment contract. Direct Internet exposure either uses appropriate TLS/authentication in-process or is constrained to a documented trusted TLS-terminating proxy/network boundary; an operator cannot accidentally interpret CORS, loopback defaults, or gRPC framing as transport security.
- [ ] **SK-13-035** Reflection, health, diagnostics, inspect/describe, schema metadata, and any future admin/debug endpoints are inventoried separately from public search. They are either intentionally public and abuse-bounded or unavailable/authorized on the public listener.

## 13.8 Request framing, body limits, decompression, and aggregate memory

- [ ] **SK-13-036** Request byte limits are enforced while bytes are being received, before unbounded buffering or semantic decoding. Chunked/streamed HTTP bodies, missing/lying `Content-Length`, arbitrary frame boundaries, and many tiny frames cannot bypass the limit.
- [ ] **SK-13-037** gRPC, gRPC-Web, HTTP/JSON, and any future public-search route have explicit request and response limits with consistent policy. A path cannot gain a larger effective limit merely by changing encoding/transport.
- [ ] **SK-13-038** Compression is either disabled for untrusted request bodies or bounded by a **post-decompression** limit and CPU budget. Test high-ratio compressed payloads, truncated compressed streams, nested/content-encoding variants, and gRPC compression negotiation. Compressed bytes alone are not a sufficient size bound.
- [ ] **SK-13-039** gRPC-Web text/base64 expansion, JSON -> FlatBuffer conversion, FlatBuffer verification/copying, IR construction, native lowering, result materialization, and response encoding are all included in the peak-memory model.
- [ ] **SK-13-040** The configured per-request byte cap and admission limit have a proven aggregate memory bound. Do not accept “each request is bounded” as proof: e.g. a 256 MiB body limit multiplied by dozens of admitted requests plus conversion copies must be reconciled with an explicit process RSS budget.
- [ ] **SK-13-041** HTTP header count/bytes, URI/path length, metadata/trailer size, and gRPC metadata are bounded by the server stack/configuration. Oversized headers fail before expensive body parsing/dispatch.
- [ ] **SK-13-042** Header-read, body-read, idle, and total request timeouts prevent slowloris clients from holding connections/admission slots indefinitely with partial requests.
- [ ] **SK-13-043** Response-size protection prevents amplification **before** unbounded response construction. A post-serialization length check is insufficient if a valid query can first allocate/materialize a huge hit/group/summary tree.
- [ ] **SK-13-044** Early EOF, malformed frames, body read failures, canceled uploads, and oversized bodies release all buffers/permits promptly and leave the next valid request healthy.

## 13.9 Semantic complexity and expensive-query DoS

- [ ] **SK-13-045** Byte-size limits are not the only DoS defense. Public requests are subject to explicit semantic complexity ceilings for all supported high-cost dimensions, with ownership of those limits clearly placed in canonical semantics/policy rather than ad-hoc transport parsing.
- [ ] **SK-13-046** Textual filter limits cover expression bytes, tokens, nesting depth, AST nodes, `IN` cardinality, and bindings; the public daemon has negative tests proving those limits survive JSON/gRPC transport unchanged.
- [ ] **SK-13-047** Typed query trees have depth/node/child/list limits. Deep `AND/OR/NOT`, phrases, alternatives/equivalents, weighted sets/WAND inputs, annotations, user-input forms, and other recursively structured query variants cannot cause stack exhaustion or superlinear parser/lowering behavior.
- [ ] **SK-13-048** Grouping/faceting/aggregation is budgeted for nesting, branches, aliases, outputs, order expressions, functions, continuation state, group/facet limits, precision, and expected fan-out. A small request must not be able to request effectively unbounded result trees or pathological grouping work.
- [ ] **SK-13-049** Pagination/projection/search-field/sort controls have server-safe ceilings. Very large `limit`, `offset`, page arithmetic, field lists, sort lists, summary requests, and hit projections cannot force disproportionate memory/CPU or overflow arithmetic.
- [ ] **SK-13-050** Tensor/vector/query-input and other binary-heavy values are bounded by dimensions, element counts, encoded bytes, and any backend-specific work limits needed to prevent small control structures from allocating enormous native state.
- [ ] **SK-13-051** Raw expert escape hatches (including YQL, raw grouping, raw sort, regex/pattern forms, generic properties, and future native-source strings) are explicitly classified for public use. Each is either disabled on the public-search route or has strict byte/complexity/cost limits and negative tests; “expert API” is not an exemption from hostile-input policy.
- [ ] **SK-13-052** Caller-provided timeout/deadline values cannot disable server protection or request an arbitrarily long execution. Effective deadlines are clamped to a server maximum and include queue wait plus the portions of parsing/dispatch/native work the contract claims to bound.
- [ ] **SK-13-053** Worst-case **valid** requests are load-tested, not only malformed inputs. Include broad/match-all searches, large legal filters, maximum legal grouping/facet fan-out, expensive ranking/query inputs, and maximum legal projection/result shapes.

## 13.10 Injection and canonical-boundary integrity

- [ ] **SK-13-054** Public request handling never builds YQL, filter syntax, grouping syntax, sort syntax, shell commands, file paths, environment variables, native config, or HTTP headers by string-interpolating untrusted bound values.
- [ ] **SK-13-055** Filter/runtime parameters remain typed values and clause-local bindings through the public transport; values are never substituted into expression source text by the daemon.
- [ ] **SK-13-056** Raw YQL/grouping/sort/property strings are passed only to their explicitly intended canonical/native parser or property channel. They cannot escape into shell execution, filesystem lookup, process configuration, header construction, or unrelated parser contexts.
- [ ] **SK-13-057** CR/LF, NUL, terminal escapes, invalid/edge Unicode, bidi/control characters, quotes, backslashes, and extremely long attacker strings cannot inject additional HTTP/gRPC headers/trailers, forge log lines, corrupt structured errors, or alter server-side configuration.
- [ ] **SK-13-058** Media type, method, path, query-string, protobuf framing, JSON framing, and gRPC-Web framing are alternate transports only. None provides an alternate semantic parser or a route around FlatBuffer verification, operation policy, the shared dispatcher, or `*_to_ir()`.
- [ ] **SK-13-059** Standard FlatBuffers JSON duplicate-field/union/presence behavior is deterministic and tested with hostile inputs. Ambiguous duplicate keys, unknown union members, numeric overflow/non-finite encodings, invalid UTF-8/escapes, and absent-vs-present-empty cases fail or normalize exactly according to the canonical bridge.
- [ ] **SK-13-060** Caller-controlled error text or backend/native messages cannot become attacker-chosen HTTP status lines/headers, gRPC status metadata, redirects, file paths, or content types.
- [ ] **SK-13-061** Every public ingress has a differential/conformance test proving that an equivalent accepted request produces the same canonical IR as the trusted direct portable path; transport-specific preprocessing cannot silently rewrite meaning.

## 13.11 Connection, admission, rate, and overload abuse

- [ ] **SK-13-062** Admission control happens before expensive body buffering/decoding/dispatch and is shared across all listeners that share the same backend/resource budget.
- [ ] **SK-13-063** Connection count itself is bounded or otherwise protected; the request semaphore is not treated as protection against clients that open many TCP/HTTP2 connections and never submit a complete request.
- [ ] **SK-13-064** Public deployment defines request-rate and burst protection. If rate limiting is intentionally delegated to a reverse proxy, direct public binding without that protection is disabled or explicitly unsupported; the acceptance test exercises the documented deployment boundary.
- [ ] **SK-13-065** One peer/HTTP2 connection cannot monopolize all admission/execution capacity indefinitely through many streams or strategically slow requests. Fairness/starvation behavior is measured under contention.
- [ ] **SK-13-066** Overload rejection is cheap, bounded, and side-effect free. Returning `429`/`RESOURCE_EXHAUSTED` must not allocate a large body, invoke the dispatcher, perform schema conversion, block on native work, or emit attacker-amplified logs.
- [ ] **SK-13-067** Queueing is explicitly bounded. There is no hidden unbounded queue in Tokio tasks, `spawn_blocking`, tonic/hyper buffering, native callbacks, or client-stream handling after the visible semaphores.
- [ ] **SK-13-068** Cancellation/deadline while waiting for admission/execution releases the correct permits and cannot later resurrect work. Repeated cancel/retry traffic cannot grow tasks, permits, threads, or native work without bound.
- [ ] **SK-13-069** Shutdown/restart under connection flood and saturated native execution completes according to the documented contract without deadlock, leaked sockets, lost Unix-socket ownership, or accepting new work after shutdown begins.
- [ ] **SK-13-070** Sustained overload/rejection tests demonstrate bounded RSS/PSS, file descriptors, sockets, Tokio tasks, blocking threads, and disk/log growth.

## 13.12 Fuzzing and adversarial generated-input verification

- [ ] **SK-13-071** Raw FlatBuffer frame fuzzing mutates/truncates/arbitrarily splices request bytes at the daemon ingress and proves: no panic, abort, UB, out-of-bounds access, unbounded allocation, hang, or backend side effect before verification succeeds.
- [ ] **SK-13-072** Schema-aware JSON fuzzing covers malformed syntax, truncation at every byte, deep arrays/objects, huge strings, escape sequences, Unicode edge cases, duplicate fields, wrong union shapes, extreme integers/floats, and valid-but-unusual canonical values.
- [ ] **SK-13-073** Structure-aware fuzz/property generators create valid portable requests and then stress recursive/high-dimensional fields: query trees, filter clauses, grouping trees, sort/projection lists, ranking inputs/properties, tensors/binary values, pagination, and raw-source escape hatches.
- [ ] **SK-13-074** HTTP body chunk-boundary fuzzing splits equivalent bodies at arbitrary byte boundaries, including one-byte frames, empty frames, delayed frames, early EOF, and read errors; semantics and bounds cannot depend on chunk layout.
- [ ] **SK-13-075** gRPC/gRPC-Web framing negative tests cover truncated length prefixes, oversized advertised frames, invalid compression flags, malformed trailers/status, base64 text corruption, unexpected content types, and protocol-version mismatches using the standard stack rather than production hand-written parsers.
- [ ] **SK-13-076** Stateful concurrency/property tests explore admission/execution permit acquisition, cancellation, deadline expiry, listener shutdown, and task completion in varied interleavings. Invariants include no permit leak/double-release and no canceled queued request executing later.
- [ ] **SK-13-077** Fuzz/stress seeds include every previously fixed daemon parsing, framing, cancellation, socket-race, and resource-exhaustion regression; new crashes/hangs become permanent minimized regression cases.
- [ ] **SK-13-078** Native-enabled daemon fuzz/hardening runs include the real dispatcher/IR/native boundary under the applicable sanitizer configuration where the generated request passes portable verification. Rust-only fuzzing is not sufficient evidence for requests that reach C++.
- [ ] **SK-13-079** After every malformed/fuzz class, a known-good request is executed against the same daemon/backend to prove liveness and state integrity; “the process did not crash” alone is insufficient.
- [ ] **SK-13-080** Fuzzers and stress tests themselves have resource/time guards so a pathological generated input is reported as a reproducible hang/cost failure instead of wedging CI indefinitely.

## 13.13 Error amplification, logging, metrics, and information disclosure

- [ ] **SK-13-081** Public 4xx/5xx and gRPC statuses do not expose filesystem paths, socket paths, environment values, native exception text, panic/backtrace data, secrets/tokens, or other deployment internals beyond the documented portable error contract.
- [ ] **SK-13-082** Error messages derived from attacker input are bounded/truncated and escaped appropriately. A tiny request cannot force an arbitrarily large reflected error response.
- [ ] **SK-13-083** Request payloads, raw YQL/filter text, bindings, document fields, tokens/credentials, and large binary/tensor values are not logged by default. Diagnostic logging that can include them is explicit, bounded, and safe for production handling.
- [ ] **SK-13-084** Logs are resistant to newline/control-character injection and flood amplification; repeated malformed requests are rate-limited/sampled or otherwise shown not to cause unbounded disk/CPU pressure.
- [ ] **SK-13-085** Metrics/tracing labels have bounded cardinality. Attacker-controlled query text, field names, document IDs, index names, user agents, raw error messages, and arbitrary property names are not used directly as unbounded metric labels.
- [ ] **SK-13-086** Health/readiness endpoints, if present, are cheap and side-effect free; they do not execute search, open indexes, force expensive native probes, or disclose sensitive configuration.
- [ ] **SK-13-087** Client-caused invalid/too-large/overload errors are distinguishable from transient server failures so well-behaved clients do not create retry storms from deterministic rejections.
- [ ] **SK-13-088** Request/trace identifiers accepted from clients are length/character bounded before logging/propagation, or are generated server-side; attacker-controlled IDs cannot become an unbounded correlation/logging channel.

## 13.14 Public-edge release gate

- [ ] **SK-13-089** The verification DAG has a stable task/profile for the public `searchkerneld` edge and selects it when public route, transport, admission, parser/codec, or daemon-limit code changes. A broad workspace test that happens to touch the daemon is not sufficient ownership.
- [ ] **SK-13-090** Exact-head live-daemon acceptance exercises every enabled public transport/route over real sockets with valid, malformed, oversized, unauthorized/disallowed-operation, timeout, cancellation, and overload cases.
- [ ] **SK-13-091** A sustained hostile-load run mixes valid search with malformed, slow, oversized, expensive-valid, canceled, and connection-flood traffic while recording latency, rejection behavior, RSS/PSS, FDs, sockets, threads/tasks, and daemon liveness.
- [ ] **SK-13-092** Recovery after hostile load is explicit: once attack traffic stops, valid requests regain normal service without daemon restart, leaked capacity, poisoned backend state, or permanently elevated resource usage.
- [ ] **SK-13-093** The public-search threat model and deployment contract state which protections live in `searchkerneld` versus an external proxy (TLS termination, authentication, per-IP rate limiting, request buffering, etc.). Any delegated protection required for safety is part of the supported deployment acceptance, not an undocumented assumption.
- [ ] **SK-13-094** No critical/high daemon-security finding is closed from source inspection alone. Closure requires a regression/negative test and the relevant live-daemon/fuzz/stress gate against the exact accepted head.

---
