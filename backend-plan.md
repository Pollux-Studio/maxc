# Backend Implementation Plan

## Overview
This plan defines phased implementation of a fully functional, efficient backend for `maxc` with terminal and browser surfaces as first-class backend concerns.

Browser baseline for this plan:
- Runtime: Chromium
- Driver: Playwright protocol
- API style: high-level `browser.*` RPC plus controlled `browser.raw.*` escape hatch

Each phase ends with two mandatory closing tasks in this order:
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
- Establish backend structure and stable JSON-RPC contracts for both terminal and browser.
- Create shared domain primitives used by all later phases.

### Tasks
- [x] Create backend module layout: `automation`, `core`, `browser`, `terminal`, `storage`, `security`, `telemetry`.
- [x] Define JSON-RPC v1 envelope schema (`id`, `method`, `params`, `result`, `error`).
- [x] Define standard error codes: `INVALID_REQUEST`, `UNAUTHORIZED`, `NOT_FOUND`, `CONFLICT`, `TIMEOUT`, `RATE_LIMITED`, `INTERNAL`.
- [x] Introduce typed IDs and core domain models (`WorkspaceId`, `PaneId`, `SurfaceId`, `SessionId`, `CommandId`, `EventId`).
- [x] Add browser domain models (`BrowserSessionId`, `BrowserTabId`, optional `FrameId`/`TargetId`) and bind them to surface/workspace abstractions.
- [x] Define `browser.*` RPC contracts for lifecycle, navigation, interaction, waits, screenshots, eval, network/cookies, uploads/downloads, tracing, subscriptions, and raw commands.
- [x] Add configuration loading with defaults for socket path, timeouts, queue limits, and logging.
- [x] Add browser runtime config defaults (Chromium executable/channel, launch options, context limits, navigation and action timeouts).
- [x] Add CI baseline for format, lint, tests, and coverage reporting.
- [x] **Phase Quality Gate (Mandatory):** review all changed code, add/update tests, run full suite, verify >=85% backend coverage.
- [x] **Commit Phase Changes (Mandatory):** create one clean phase-scoped commit (example: `phase-1: add browser contracts and runtime config`).

### Deliverables
- Compiling backend skeleton including `browser` module
- Contract and type definitions for terminal and browser
- Baseline CI checks

### Exit Criteria
- All contract tests pass
- No lint/format warnings

---

## Phase 2: IPC RPC Server and Security

### Goals
- Provide secure local control plane with authenticated JSON-RPC for terminal and browser operations.

### Tasks
- [x] Implement async local IPC server (Windows named pipe) with Tokio.
- [x] Add JSON-RPC router and method dispatcher with middleware chain.
- [x] Implement session APIs: `session.create`, `session.refresh`, `session.revoke`.
- [x] Add browser route registration and dispatch for `browser.*` and `browser.raw.*`.
- [x] Enforce OS ACL and token validation for mutating endpoints.
- [x] Add method-level authorization policy for browser mutating operations and raw-command access.
- [x] Add input validation, request size limits, and per-client/global rate limits.
- [x] Add stricter limits for browser-heavy operations (screenshots, downloads, raw commands, subscriptions).
- [x] Add request timeouts, cancellation handling, and correlation IDs.
- [x] Add deterministic error mapping from internal errors to JSON-RPC errors.
- [x] Add explicit audit fields for browser commands (`workspace_id`, `surface_id`, `browser_session_id`, `tab_id`).
- [x] **Phase Quality Gate (Mandatory):** review all changed code, add/update tests, run full suite, verify >=85% backend coverage.
- [x] **Commit Phase Changes (Mandatory):** create one clean phase-scoped commit (example: `phase-2: secure browser rpc routing`).

### Deliverables
- Running authenticated IPC RPC server with browser namespaces
- Security middleware and hardened error handling for browser operations

### Exit Criteria
- Unauthorized mutations are blocked
- Valid terminal and browser requests succeed under concurrent clients

---

## Phase 3: Event Store and State Recovery

### Goals
- Add durable backend state and deterministic recovery for terminal and browser flows.

### Tasks
- [x] Implement append-only event store with segment files and checksums.
- [x] Define event schema with version field for forward compatibility.
- [x] Extend event schema with browser event types and payload versions.
- [x] Implement event index for efficient reads and replay positions.
- [x] Build projection engine that reconstructs in-memory state from events.
- [x] Extend projections for browser sessions, tabs, and automation state.
- [x] Add snapshotting and startup recovery (load snapshot + replay tail).
- [x] Include browser state in snapshots and replay validation.
- [x] Implement command idempotency keyed by `CommandId`.
- [x] Extend idempotency handling for browser lifecycle and automation commands.
- [x] Add compaction policy for old segments and snapshot rotation.
- [x] **Phase Quality Gate (Mandatory):** review all changed code, add/update tests, run full suite, verify >=85% backend coverage.
- [x] **Commit Phase Changes (Mandatory):** create one clean phase-scoped commit (example: `phase-3: persist browser events and recovery`).

### Deliverables
- Durable event log and projection engine including browser state
- Crash recovery with idempotent terminal and browser replay

### Exit Criteria
- Restart after crash restores consistent terminal and browser state
- Duplicate command replay does not duplicate side effects

---

## Phase 4: Execution Core (Terminal + Browser)

### Goals
- Deliver robust terminal and browser lifecycle control with low-latency streaming.

### Tasks
- [x] Implement ConPTY-backed terminal lifecycle APIs: `terminal.spawn`, `terminal.input`, `terminal.resize`, `terminal.kill`.
- [x] Implement Chromium session manager through Playwright driver.
- [x] Implement browser lifecycle APIs: `browser.create`, `browser.attach`, `browser.detach`, `browser.close`.
- [x] Implement tab/page APIs: `browser.tab.open`, `browser.tab.list`, `browser.tab.focus`, `browser.tab.close`.
- [x] Implement navigation APIs: `browser.goto`, `browser.reload`, `browser.back`, `browser.forward`.
- [x] Implement automation APIs: DOM query, click, type, key events, waits, screenshot, evaluate script.
- [x] Implement advanced APIs: network interception, cookie/storage controls, file upload/download, tracing.
- [x] Implement controlled raw-command API under `browser.raw.*` with policy and limits.
- [x] Add surface/workspace binding between terminal and browser sessions and state model.
- [x] Implement streaming subscriptions (`terminal.subscribe`, `browser.subscribe`) with fan-out.
- [x] Add backpressure and per-subscriber buffering to prevent global stalls.
- [x] Add scheduler for interactive vs background workloads with fairness rules.
- [x] Add lifecycle cleanup hooks to prevent orphaned sessions/processes.
- [x] Add robust error propagation for process exits, runtime failures, IO failures, and cancellation.
- [x] **Phase Quality Gate (Mandatory):** review all changed code, add/update tests, run full suite, verify >=85% backend coverage.
- [x] **Commit Phase Changes (Mandatory):** create one clean phase-scoped commit (example: `phase-4: implement browser and terminal execution core`).

### Deliverables
- Functional terminal and browser backend APIs
- Stable streaming and lifecycle behavior across both surface types

### Exit Criteria
- Multi-session interaction remains responsive
- Output/event ordering and session ownership are correct

---

## Phase 5: Performance and Reliability Hardening

### Goals
- Optimize latency/throughput and harden backend against failure scenarios.

### Tasks
- [x] Define backend SLOs and benchmark profiles for terminal and browser workloads.
- [x] Add benchmark harness for RPC latency, stream latency, command throughput, and browser action latency.
- [x] Optimize hot paths (buffer reuse, lock contention reduction, batched persistence, event fan-out).
- [x] Add graceful shutdown with queue draining and in-flight request handling.
- [x] Add fault injection tests for partial writes, process crashes, browser runtime crashes, and restart loops.
- [x] Add protective controls: circuit breaker, queue caps, overload rejection policy.
- [x] Add regression guardrails in CI for key performance thresholds.
- [x] **Phase Quality Gate (Mandatory):** review all changed code, add/update tests, run full suite, verify >=85% backend coverage.
- [x] **Commit Phase Changes (Mandatory):** create one clean phase-scoped commit (example: `phase-5: harden browser and terminal reliability`).

### Deliverables
- Performance baselines and optimization changes
- Reliability controls and failure-handling coverage across terminal and browser paths

### Exit Criteria
- SLO targets are met in benchmark runs
- Crash/recovery tests pass consistently

---

## Phase 6: Operability and Release Readiness

### Goals
- Make backend observable, diagnosable, and release-safe for terminal and browser operations.

### Tasks
- [ ] Add structured logs with `correlation_id`, `session_id`, `command_id`, and component tags.
- [ ] Add browser-specific fields in logs (`browser_session_id`, `tab_id`, `target_id`, `surface_id`).
- [ ] Add metrics (latency histograms, queue depth, error rate, active sessions, replay time).
- [ ] Add browser metrics (navigation latency, action latency, event stream lag, crash count, reconnect count).
- [ ] Add OpenTelemetry traces for RPC path, dispatcher, storage, terminal, and browser execution.
- [ ] Implement health/readiness endpoints and diagnostics commands.
- [ ] Finalize CLI-to-RPC integration for core terminal and browser workflows.
- [ ] Add compatibility tests for JSON-RPC v1 stability and additive evolution rules.
- [ ] Produce operational runbook: startup, shutdown, incident triage, recovery steps.
- [ ] **Phase Quality Gate (Mandatory):** review all changed code, add/update tests, run full suite, verify >=85% backend coverage.
- [ ] **Commit Phase Changes (Mandatory):** create one clean phase-scoped commit (example: `phase-6: release browser-enabled backend`).

### Deliverables
- Production-grade observability for terminal and browser subsystems
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
- Core terminal and browser backend flows are functional, tested, and observable.
