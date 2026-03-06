# Backend Implementation Plan

## Overview
This plan defines phased implementation of a fully functional, efficient, and optimized backend for `maxc` (terminal-first).  
Each phase ends with two mandatory closing tasks in this exact order:
1. `Phase Quality Gate (Mandatory)`
2. `Commit Phase Changes (Mandatory)`

All quality gates must enforce:
- Review all code changed in the phase
- Add or update tests for all changed behavior
- Run full test suite
- Verify backend test coverage is at least 85%

Global validation commands for every quality gate:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo llvm-cov --workspace --all-features --fail-under-lines 85
```

---

## Phase 1: Foundation and Contracts

### Goals
- Establish backend project structure and stable API contracts.
- Create shared domain primitives used by all later phases.

### Tasks
- [x] Create backend module layout: `automation`, `core`, `terminal`, `storage`, `security`, `telemetry`.
- [x] Define JSON-RPC v1 envelope schema (`id`, `method`, `params`, `result`, `error`).
- [x] Define standard error codes: `INVALID_REQUEST`, `UNAUTHORIZED`, `NOT_FOUND`, `CONFLICT`, `TIMEOUT`, `RATE_LIMITED`, `INTERNAL`.
- [x] Introduce typed IDs and core domain models (`WorkspaceId`, `PaneId`, `SurfaceId`, `SessionId`, `CommandId`, `EventId`).
- [x] Add configuration loading with defaults for socket path, timeouts, queue limits, and logging.
- [x] Add CI baseline for format, lint, tests, and coverage reporting.
- [x] **Phase Quality Gate (Mandatory):** review all changed code, add/update tests, run full suite, verify >=85% backend coverage.
- [x] **Commit Phase Changes (Mandatory):** create one clean phase-scoped commit (example: `phase-1: add backend contracts and core types`).

### Deliverables
- Compiling backend skeleton
- Contract and type definitions
- Baseline CI checks

### Exit Criteria
- All contract tests pass
- No lint/format warnings

---

## Phase 2: IPC RPC Server and Security

### Goals
- Provide secure local control plane with authenticated JSON-RPC.

### Tasks
- [x] Implement async local IPC server (Windows named pipe) with Tokio.
- [x] Add JSON-RPC router and method dispatcher with middleware chain.
- [x] Implement session APIs: `session.create`, `session.refresh`, `session.revoke`.
- [x] Enforce OS ACL and token validation for all mutating endpoints.
- [x] Add input validation, request size limits, and per-client/global rate limits.
- [x] Add request timeouts, cancellation handling, and correlation IDs.
- [x] Add deterministic error mapping from internal errors to JSON-RPC errors.
- [x] **Phase Quality Gate (Mandatory):** review all changed code, add/update tests, run full suite, verify >=85% backend coverage.
- [x] **Commit Phase Changes (Mandatory):** create one clean phase-scoped commit (example: `phase-2: add secure local rpc server`).

### Deliverables
- Running authenticated IPC RPC server
- Security middleware and hardened error handling

### Exit Criteria
- Unauthorized mutations are blocked
- Valid requests succeed under concurrent clients

---

## Phase 3: Event Store and State Recovery

### Goals
- Add durable backend state and deterministic recovery.

### Tasks
- [ ] Implement append-only event store with segment files and checksums.
- [ ] Define event schema with version field for forward compatibility.
- [ ] Implement event index for efficient reads and replay positions.
- [ ] Build projection engine that reconstructs in-memory state from events.
- [ ] Add snapshotting and startup recovery (load snapshot + replay tail).
- [ ] Implement command idempotency keyed by `CommandId`.
- [ ] Add compaction policy for old segments and snapshot rotation.
- [ ] **Phase Quality Gate (Mandatory):** review all changed code, add/update tests, run full suite, verify >=85% backend coverage.
- [ ] **Commit Phase Changes (Mandatory):** create one clean phase-scoped commit (example: `phase-3: add event store and deterministic replay`).

### Deliverables
- Durable event log and projection engine
- Crash recovery with idempotent replay

### Exit Criteria
- Restart after crash restores consistent state
- Duplicate command replay does not duplicate side effects

---

## Phase 4: Terminal Execution Core

### Goals
- Deliver robust terminal lifecycle and low-latency output streaming.

### Tasks
- [ ] Implement ConPTY-backed terminal lifecycle APIs: `terminal.spawn`, `terminal.input`, `terminal.resize`, `terminal.kill`.
- [ ] Add surface/workspace binding between terminal sessions and state model.
- [ ] Implement output streaming subscriptions (`terminal.subscribe`) with fan-out.
- [ ] Add backpressure and per-subscriber buffering to prevent global stalls.
- [ ] Add scheduler for interactive vs background workloads with fairness rules.
- [ ] Add lifecycle cleanup hooks to prevent orphaned sessions/processes.
- [ ] Add robust error propagation for process exits, IO failures, and cancellation.
- [ ] **Phase Quality Gate (Mandatory):** review all changed code, add/update tests, run full suite, verify >=85% backend coverage.
- [ ] **Commit Phase Changes (Mandatory):** create one clean phase-scoped commit (example: `phase-4: implement terminal core and streaming`).

### Deliverables
- Functional terminal backend APIs
- Stable streaming and process lifecycle behavior

### Exit Criteria
- Multi-session interaction remains responsive
- Output ordering and session ownership are correct

---

## Phase 5: Performance and Reliability Hardening

### Goals
- Optimize latency/throughput and harden against failure scenarios.

### Tasks
- [ ] Define backend SLOs and benchmark profiles (interactive and batch workloads).
- [ ] Add benchmark harness for RPC latency, stream latency, and command throughput.
- [ ] Optimize hot paths (buffer reuse, lock contention reduction, batched persistence).
- [ ] Add graceful shutdown with queue draining and in-flight request handling.
- [ ] Add fault injection tests for partial writes, process crashes, and restart loops.
- [ ] Add protective controls: circuit breaker, queue caps, overload rejection policy.
- [ ] Add regression guardrails in CI for key performance thresholds.
- [ ] **Phase Quality Gate (Mandatory):** review all changed code, add/update tests, run full suite, verify >=85% backend coverage.
- [ ] **Commit Phase Changes (Mandatory):** create one clean phase-scoped commit (example: `phase-5: harden backend performance and recovery`).

### Deliverables
- Performance baselines and optimization changes
- Reliability controls and failure-handling coverage

### Exit Criteria
- SLO targets are met in benchmark runs
- Crash/recovery tests pass consistently

---

## Phase 6: Operability and Release Readiness

### Goals
- Make backend observable, diagnosable, and release-safe.

### Tasks
- [ ] Add structured logs with `correlation_id`, `session_id`, `command_id`, and component tags.
- [ ] Add metrics (latency histograms, queue depth, error rate, active sessions, replay time).
- [ ] Add OpenTelemetry traces for RPC path, dispatcher, storage, and terminal execution.
- [ ] Implement health/readiness endpoints and diagnostics commands.
- [ ] Finalize CLI-to-RPC integration for core workflows (`list-workspaces`, `new-workspace`, `send`, subscriptions).
- [ ] Add compatibility tests for JSON-RPC v1 stability and additive evolution rules.
- [ ] Produce operational runbook: startup, shutdown, incident triage, recovery steps.
- [ ] **Phase Quality Gate (Mandatory):** review all changed code, add/update tests, run full suite, verify >=85% backend coverage.
- [ ] **Commit Phase Changes (Mandatory):** create one clean phase-scoped commit (example: `phase-6: add observability and release readiness`).

### Deliverables
- Production-grade observability
- Release checklist and stable backend interfaces

### Exit Criteria
- Health and diagnostics validated
- Release checklist completed with all checks passing

---

## Commit Policy (Applies to Every Phase)
- Commit only after phase quality gate passes.
- Keep each phase commit scoped to that phase only.
- Use clear imperative messages: `phase-<N>: <short outcome>`.
- Do not mix changes from multiple phases in one commit.

## Done Definition (Program-Level)
- All 6 phases completed in order.
- Each phase includes passing quality gate and final commit.
- Backend coverage remains >=85% after every phase.
- Core terminal-first backend flows are functional, tested, and observable.
