# 13. Daemon/RPC host — `searchkerneld`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + UNIT + PKG`; transport behavior requires real daemon execution; malformed/oversize/socket-race behavior requires `NEG`; backend semantics are not proven here.

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

## 13.5 TCP/gRPC-Web

- [ ] **SK-13-023** TCP listener exposes the same dispatcher semantics as UDS.
- [ ] **SK-13-024** Optional gRPC-Web listener shares the same dispatcher and execution bound.
- [ ] **SK-13-025** gRPC-Web does not introduce JSON/domain semantics.
- [ ] **SK-13-026** CORS/HTTP behavior is explicitly configured/tested where relevant.

## 13.6 Generated RPC freshness

- [ ] **SK-13-027** Checked-in Rust RPC generated module matches fresh standard-tool generation.
- [ ] **SK-13-028** RPC generation is deterministic and pinned.

---
